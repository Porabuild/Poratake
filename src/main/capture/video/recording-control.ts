import { dialog, BrowserWindow, shell } from 'electron';
import { daemon } from '@/main/daemon';
import { getConfig, updateConfig } from '@/main/settings';
import { showCameraPreview, hideCameraPreview } from './camera-preview';
import {
  startPendingRecording,
  cancelPendingRecording,
  stopRecordingAction,
  deleteRecordingAction,
  restartRecordingAction,
} from './recording-actions';
import { pauseRecording, resumeRecording } from './recorder';
import { checkAndRequestMicrophonePermission } from './permissions';
import {
  getCameraStatus,
  requestCameraPermission,
  openCameraPreferences,
} from '@/main/system/permissions';
import type { AspectRatio } from '@/types/aspect-ratio';
import {
  setAreaSelectorAspectRatio,
  hideAreaSelector,
  showAreaSelector,
} from '@/main/capture/area-selector';

async function checkAndRequestCameraPermission(): Promise<boolean> {
  const status = getCameraStatus();

  if (
    status === 'not-determined' ||
    status === 'denied' ||
    status === 'restricted'
  ) {
    const granted = await requestCameraPermission();
    if (!granted) {
      const options = {
        type: 'error' as const,
        title: 'Camera Permission Required',
        message: 'Camera access is not granted.',
        detail:
          'To record with camera, please grant camera permission in System Settings.\n\n' +
          'Go to: System Settings > Privacy & Security > Camera\n' +
          'Enable access for Capty',
        buttons: ['Open Settings', 'Cancel'],
        defaultId: 0,
      };

      const win = BrowserWindow.getFocusedWindow();
      const result = win
        ? await dialog.showMessageBox(win, options)
        : await dialog.showMessageBox(options);

      if (result.response === 0) {
        openCameraPreferences();
      }
      return false;
    }
  }

  return true;
}

type RecordingControlMode = 'pre-recording' | 'recording';

let eventCleanup: (() => void) | null = null;
let timerInterval: NodeJS.Timeout | null = null;
let recordingStartTime: number | null = null;
let pausedElapsedTime: number = 0;
let isPaused: boolean = false;
let currentMode: RecordingControlMode = 'pre-recording';

let currentAreaSelection: {
  x: number;
  y: number;
  width: number;
  height: number;
} | null = null;

export function getCurrentRecordingAreaSelection() {
  return currentAreaSelection;
}

const WINDOW_WIDTH_PRE_RECORDING = 496;
const WINDOW_WIDTH_RECORDING = 352;
const WINDOW_HEIGHT = 48;

function getWindowWidth(): number {
  return currentMode === 'pre-recording'
    ? WINDOW_WIDTH_PRE_RECORDING
    : WINDOW_WIDTH_RECORDING;
}

function calculateCenteredPosition(area: {
  x: number;
  y: number;
  width: number;
  height: number;
}): { x: number; y: number } {
  const windowWidth = getWindowWidth();
  const x = Math.round(area.x + area.width / 2 - windowWidth / 2);
  const y = Math.round(area.y + area.height / 2 - WINDOW_HEIGHT / 2);
  return { x, y };
}

function getRecordingSettings() {
  const config = getConfig();
  return {
    systemAudio: config.recording.systemAudio,
    micEnabled: config.recording.micEnabled,
    micMuted: micMuted,
    selectedMicId: config.recording.selectedMicId,
    selectedMicName: config.recording.selectedMicName,
    cameraEnabled: config.recording.camera?.enabled ?? false,
    selectedCameraId: config.recording.camera?.selectedDeviceId,
    selectedCameraName: config.recording.camera?.selectedDeviceName,
    cameraSize: config.recording.camera?.size ?? 'medium',
    cameraShape: config.recording.camera?.shape ?? 'circle',
    cameraFlipped: config.recording.camera?.flipped ?? false,
    selectedIOSDeviceId: config.recording.iosDevice?.id ?? null,
    selectedIOSDeviceName: config.recording.iosDevice?.name ?? null,
  };
}

interface EventData {
  deviceId?: string | null;
  deviceName?: string | null;
  size?: string;
  shape?: string;
  width?: number;
  height?: number;
  name?: string;
}

let micMuted: boolean = false;
let isHandlingStart: boolean = false;

async function handleEvent(event: string, data?: EventData): Promise<void> {
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
    case 'recording-control:toggle-mic-mute':
      await handleToggleMicMute();
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
    case 'recording-control:select-aspect-ratio':
      await handleSelectAspectRatio(data);
      break;
    case 'recording-control:start':
      await handleStart();
      break;
    case 'recording-control:cancel':
      handleCancel();
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
    case 'recording-control:restart':
      await handleRestart();
      break;
    case 'recording-control:delete':
      await handleDelete();
      break;
    case 'recording-control:open-ios-help':
      handleOpenIOSHelp();
      break;
    case 'recording-control:select-ios-device':
      await handleSelectIOSDevice(
        data?.deviceId ?? null,
        data?.deviceName ?? null
      );
      break;
  }
}

async function handleSelectAspectRatio(data?: EventData): Promise<void> {
  if (!data || data.width === undefined || data.height === undefined) return;

  const ratio: AspectRatio = {
    name: data.name ?? 'Custom',
    width: data.width,
    height: data.height,
  };

  const config = getConfig();
  if (config.recording.iosDevice?.id) {
    updateConfig({
      recording: {
        ...config.recording,
        iosDevice: null,
      },
    });
    await updateNativeSettings();
  }

  await showAreaSelector();
  await setAreaSelectorAspectRatio(ratio);
}

async function handleToggleSystemAudio(): Promise<void> {
  const config = getConfig();
  const newSystemAudio = !config.recording.systemAudio;

  updateConfig({
    recording: {
      ...config.recording,
      systemAudio: newSystemAudio,
    },
  });

  await updateNativeSettings();
}

async function handleToggleMic(): Promise<void> {
  const config = getConfig();
  const newMicEnabled = !config.recording.micEnabled;

  if (newMicEnabled) {
    const hasPermission = await checkAndRequestMicrophonePermission();
    if (!hasPermission) return;
  }

  updateConfig({
    recording: {
      ...config.recording,
      micEnabled: newMicEnabled,
    },
  });

  await updateNativeSettings();
}

async function handleSelectMic(
  deviceId: string | null,
  deviceName: string | null
): Promise<void> {
  const config = getConfig();

  if (deviceId === null) {
    updateConfig({
      recording: {
        ...config.recording,
        micEnabled: false,
        selectedMicId: null,
        selectedMicName: null,
      },
    });
  } else {
    const hasPermission = await checkAndRequestMicrophonePermission();
    if (!hasPermission) return;

    updateConfig({
      recording: {
        ...config.recording,
        micEnabled: true,
        selectedMicId: deviceId,
        selectedMicName: deviceName,
      },
    });
  }

  await updateNativeSettings();
}

async function handleToggleCamera(): Promise<void> {
  const config = getConfig();
  const newCameraEnabled = !config.recording.camera?.enabled;

  if (newCameraEnabled) {
    const hasPermission = await checkAndRequestCameraPermission();
    if (!hasPermission) return;
  }

  const updatedCamera = {
    ...config.recording.camera,
    enabled: newCameraEnabled,
  };

  updateConfig({
    recording: {
      ...config.recording,
      camera: updatedCamera,
    },
  });

  if (newCameraEnabled && updatedCamera) {
    showCameraPreview(updatedCamera);
  } else {
    hideCameraPreview();
  }

  await updateNativeSettings();
}

async function handleSelectCamera(
  deviceId: string | null,
  deviceName: string | null
): Promise<void> {
  const config = getConfig();

  if (deviceId === null) {
    const updatedCamera = {
      ...config.recording.camera,
      enabled: false,
      selectedDeviceId: null,
      selectedDeviceName: null,
    };

    updateConfig({
      recording: {
        ...config.recording,
        camera: updatedCamera,
      },
    });

    hideCameraPreview();
  } else {
    const hasPermission = await checkAndRequestCameraPermission();
    if (!hasPermission) return;

    const updatedCamera = {
      ...config.recording.camera,
      enabled: true,
      selectedDeviceId: deviceId,
      selectedDeviceName: deviceName,
    };

    updateConfig({
      recording: {
        ...config.recording,
        camera: updatedCamera,
      },
    });

    showCameraPreview(updatedCamera);
  }

  await updateNativeSettings();
}

async function handleToggleMicMute(): Promise<void> {
  micMuted = !micMuted;

  try {
    await daemon.call('screen-recorder', 'setMicMuted', { muted: micMuted });
  } catch (error) {
    console.error('Failed to set mic muted state:', error);
  }
}

async function handleStart(): Promise<void> {
  if (isHandlingStart) return;
  isHandlingStart = true;

  const config = getConfig();

  await daemon.call('recording-control', 'updateState', {
    isStarting: true,
    isPaused: false,
  });

  try {
    await startPendingRecording({
      systemAudio: config.recording.systemAudio,
      micEnabled: config.recording.micEnabled,
      micDeviceId: config.recording.micEnabled
        ? config.recording.selectedMicId
        : null,
      micDeviceName: config.recording.micEnabled
        ? config.recording.selectedMicName
        : null,
      cameraEnabled: config.recording.camera?.enabled ?? false,
      cameraDeviceId: config.recording.camera?.enabled
        ? config.recording.camera?.selectedDeviceId
        : null,
      cameraDeviceName: config.recording.camera?.enabled
        ? config.recording.camera?.selectedDeviceName
        : null,
      keyboardEnabled: true,
      iosDeviceId: config.recording.iosDevice?.id ?? null,
      iosDeviceName: config.recording.iosDevice?.name ?? null,
    });
  } finally {
    await daemon.call('recording-control', 'updateState', {
      isStarting: false,
      isPaused,
    });
    isHandlingStart = false;
  }
}

function handleCancel(): void {
  cancelPendingRecording();
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
  await stopRecordingAction();
}

async function handleRestart(): Promise<void> {
  await restartRecordingAction();
}

async function handleDelete(): Promise<void> {
  await deleteRecordingAction();
}

function handleOpenIOSHelp(): void {
  shell.openExternal(
    'https://capty.app/blog/record-your-iphone-or-ipad-screen-on-mac'
  );
  cancelPendingRecording();
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

  await updateNativeSettings();
}

async function updateNativeSettings(): Promise<void> {
  const settings = getRecordingSettings();

  try {
    await daemon.call('recording-control', 'updateSettings', settings);
  } catch (error) {
    console.error('Failed to update recording control settings:', error);
  }
}

function setupEventListener(): void {
  if (eventCleanup) return;

  const handler = (event: string, data?: unknown) => {
    handleEvent(event, data as EventData);
  };

  daemon.onEvent(handler);
  eventCleanup = () => daemon.offEvent(handler);
}

function cleanupEventListener(): void {
  eventCleanup?.();
  eventCleanup = null;
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

  daemon.call('recording-control', 'updateState', { isPaused: true });
}

export function resumeTimer(): void {
  if (!isPaused) return;

  isPaused = false;
  recordingStartTime = Date.now() - pausedElapsedTime * 1000;

  timerInterval = setInterval(() => {
    sendElapsedTime();
  }, 1000);

  sendElapsedTime();

  daemon.call('recording-control', 'updateState', { isPaused: false });
}

function sendElapsedTime(): void {
  if (recordingStartTime === null) return;

  const elapsedMs = Date.now() - recordingStartTime;
  const elapsedSeconds = Math.floor(elapsedMs / 1000);

  daemon.call('recording-control', 'updateTimer', { seconds: elapsedSeconds });
}

export function prewarmRecordingControlWindow(): void {}

export function showPreRecordingControl(area?: {
  x: number;
  y: number;
  width: number;
  height: number;
}): void {
  currentAreaSelection = area || null;
  currentMode = 'pre-recording';
  setupEventListener();

  const config = getConfig();
  if (config.recording.camera?.enabled) {
    showCameraPreview(config.recording.camera);
  }

  const position = area ? calculateCenteredPosition(area) : { x: 100, y: 100 };

  daemon
    .call('recording-control', 'show', {
      ...position,
      mode: 'pre-recording',
      settings: getRecordingSettings(),
    })
    .catch(error => {
      console.error('Failed to show recording control:', error);
    });
}

export function updateRecordingControlPosition(area: {
  x: number;
  y: number;
  width: number;
  height: number;
}): void {
  currentAreaSelection = area;

  const position = calculateCenteredPosition(area);

  daemon.call('recording-control', 'update', position).catch(error => {
    console.error('Failed to update recording control position:', error);
  });
}

export async function hidePreRecordingControl(
  hideCamera: boolean = true
): Promise<void> {
  cleanupEventListener();

  currentAreaSelection = null;
  currentMode = 'pre-recording';

  if (hideCamera) {
    hideCameraPreview();
  }

  try {
    await daemon.call('recording-control', 'hide');
  } catch (error) {
    console.error('Failed to hide recording control:', error);
  }
}

export async function showRecordingControl(): Promise<void> {
  currentMode = 'recording';
  micMuted = false;
  setupEventListener();

  try {
    await daemon.call('recording-control', 'setMode', {
      mode: 'recording',
    });
  } catch (error) {
    console.error('Failed to switch to recording mode:', error);
  }

  startTimer();
}

export async function hideRecordingControl(): Promise<void> {
  stopTimer();
  cleanupEventListener();
  micMuted = false;

  currentAreaSelection = null;
  currentMode = 'pre-recording';

  try {
    await daemon.call('recording-control', 'hide');
  } catch (error) {
    console.error('Failed to hide recording control:', error);
  }
}

export function getRecordingControlWindow(): null {
  return null;
}
