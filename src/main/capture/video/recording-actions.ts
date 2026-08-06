import {
  showRecordingControl,
  hideRecordingControl,
  hidePreRecordingControl,
  prewarmRecordingControlWindow,
  showPreRecordingControl,
  updateRecordingControlPosition,
} from './recording-control.ts';
import {
  showCameraPreview,
  hideCameraPreview,
  enableCameraContentProtection,
  disableCameraContentProtection,
} from './camera-preview.ts';
import {
  startAreaSelection,
  confirmAreaSelection,
  cancelAreaSelection,
  killAreaSelector,
} from '@/main/capture/area-selector';
import {
  isRecording,
  getRecordingDuration,
  getCurrentRecordingPath,
  startRecordingWithConfig,
  stopRecording,
  createRecordingProject,
  prewarmRecorder,
} from './recorder.ts';
import { prewarmOverlay } from './overlay.ts';
import { createVideoEditorWindow } from './video-editor.ts';
import { showCapturePreview } from '@/main/capture/capture-preview';
import { addToHistory } from '@/main/history';
import {
  showRecordingError,
  checkAndRequestMicrophonePermission,
} from './permissions.ts';
import { deleteVideo } from './delete-video.ts';
import { getConfig, updateConfig } from '@/main/settings';
import { generateInitialEditorState } from './auto-zoom-generator.ts';
import type { RecordingConfig, RecordingType } from '@/types/video.ts';

let lastRecordingConfig: RecordingConfig | null = null;
let currentRecordingType: RecordingType | undefined;

function clearSelectedIOSDevice(): void {
  const config = getConfig();

  if (!config.recording.iosDevice) {
    return;
  }

  updateConfig({
    recording: {
      ...config.recording,
      iosDevice: null,
    },
  });
}

export interface RecordingOptions {
  systemAudio?: boolean;
  micEnabled?: boolean;
  micDeviceId?: string | null;
  micDeviceName?: string | null;
  cameraEnabled?: boolean;
  cameraDeviceId?: string | null;
  cameraDeviceName?: string | null;
  keyboardEnabled?: boolean;
  iosDeviceId?: string | null;
  iosDeviceName?: string | null;
}

export async function stopRecordingAction(): Promise<string | null> {
  const duration = getRecordingDuration();
  let outputPath: string | null = null;

  try {
    outputPath = await stopRecording(hideRecordingControl);
  } finally {
    disableCameraContentProtection();
    hideCameraPreview();
    hideRecordingControl();
  }

  if (outputPath) {
    const historyItem = await addToHistory(outputPath, 'video', duration);
    await generateInitialEditorState({
      projectPath: outputPath,
      recordingType: currentRecordingType,
    });

    const config = getConfig();
    if (config.recording.showPreview) {
      showCapturePreview(outputPath, 'video', historyItem?.id);
    } else {
      createVideoEditorWindow(outputPath);
    }
  }

  currentRecordingType = undefined;

  return outputPath;
}

export async function recordArea(): Promise<void> {
  if (isRecording()) {
    console.log('Already recording');
    return;
  }

  clearSelectedIOSDevice();

  prewarmRecordingControlWindow();
  prewarmRecorder();
  prewarmOverlay();

  try {
    await startAreaSelection({
      onSelected: selection => {
        const bounds =
          selection.x !== undefined &&
          selection.y !== undefined &&
          selection.width !== undefined &&
          selection.height !== undefined
            ? {
                x: selection.x,
                y: selection.y,
                width: selection.width,
                height: selection.height,
              }
            : undefined;
        showPreRecordingControl(bounds);
      },
      onUpdate: selection => {
        if (
          selection.status === 'updated' &&
          selection.x !== undefined &&
          selection.y !== undefined &&
          selection.width !== undefined &&
          selection.height !== undefined
        ) {
          updateRecordingControlPosition({
            x: selection.x,
            y: selection.y,
            width: selection.width,
            height: selection.height,
          });
        }
      },
      onCancelled: () => {
        hidePreRecordingControl();
      },
    });
  } catch (error) {
    console.error('Error starting area selection:', error);
  }
}

export async function startPendingRecording(
  options: RecordingOptions = {}
): Promise<void> {
  const iosDeviceId = options.iosDeviceId ?? null;
  const iosDeviceName = options.iosDeviceName ?? null;
  const isIOSRecording = iosDeviceId !== null;

  currentRecordingType = isIOSRecording ? 'ios-device' : undefined;

  if (isIOSRecording) {
    await killAreaSelector();
  }

  let x: number | undefined;
  let y: number | undefined;
  let width: number | undefined;
  let height: number | undefined;
  let screenId: number | undefined;

  if (!isIOSRecording) {
    const selection = await confirmAreaSelection();

    if (!selection || selection.status === 'cancelled') {
      console.log('No pending selection to record');
      return;
    }

    x = selection.x;
    y = selection.y;
    width = selection.width;
    height = selection.height;
    screenId = selection.screenId;

    if (
      x === undefined ||
      y === undefined ||
      width === undefined ||
      height === undefined
    ) {
      console.error('Invalid area selection');
      return;
    }
  }

  const micEnabled = options.micEnabled ?? false;

  if (micEnabled) {
    const granted = await checkAndRequestMicrophonePermission();
    if (!granted) {
      return;
    }
  }

  if (!isIOSRecording) {
    await hidePreRecordingControl(false);
  }

  const outputPath = createRecordingProject();
  const includeAudio = options.systemAudio ?? true;
  const micDeviceId = options.micDeviceId ?? null;
  const micDeviceName = options.micDeviceName ?? null;
  const cameraEnabled = options.cameraEnabled ?? false;
  const cameraDeviceId = options.cameraDeviceId ?? null;
  const cameraDeviceName = options.cameraDeviceName ?? null;
  const keyboardEnabled = isIOSRecording
    ? false
    : (options.keyboardEnabled ?? false);

  if (cameraEnabled) {
    enableCameraContentProtection();
  }

  lastRecordingConfig = {
    x,
    y,
    width,
    height,
    displayId: screenId,
    includeAudio,
    micEnabled,
    micDeviceId,
    micDeviceName,
    cameraEnabled,
    cameraDeviceId,
    cameraDeviceName,
    keyboardEnabled,
    frameRate: 60,
    outputPath,
    iosDeviceId,
    iosDeviceName,
  };

  try {
    await startRecordingWithConfig(
      {
        x,
        y,
        width,
        height,
        displayId: screenId,
        includeAudio,
        micEnabled,
        micDeviceId,
        micDeviceName,
        cameraEnabled,
        cameraDeviceId,
        cameraDeviceName,
        keyboardEnabled,
        frameRate: 60,
        outputPath,
        iosDeviceId,
        iosDeviceName,
      },
      showRecordingControl
    );

    console.log('Recording started:', {
      x,
      y,
      width,
      height,
      includeAudio,
      micEnabled,
      micDeviceName,
      cameraEnabled,
      cameraDeviceName,
      keyboardEnabled,
      iosDeviceId,
      iosDeviceName,
    });
  } catch (error) {
    console.error('Error starting recording:', error);
    await hideRecordingControl();
    disableCameraContentProtection();
    hideCameraPreview();
    if (error instanceof Error) {
      await showRecordingError(error);
    }
  }
}

export async function cancelPendingRecording(): Promise<void> {
  cancelAreaSelection();
  hidePreRecordingControl();
}

export async function deleteRecordingAction(): Promise<void> {
  const currentPath = getCurrentRecordingPath();

  try {
    await stopRecording(hideRecordingControl);
  } finally {
    currentRecordingType = undefined;
    disableCameraContentProtection();
    hideCameraPreview();
    hideRecordingControl();
  }

  if (currentPath) {
    await deleteVideo(currentPath, { showNotification: true });
  }
}

export async function restartRecordingAction(): Promise<void> {
  if (!lastRecordingConfig) {
    console.error('No previous recording config to restart');
    return;
  }

  const config = { ...lastRecordingConfig };
  const currentPath = getCurrentRecordingPath();

  try {
    await stopRecording(hideRecordingControl);
  } finally {
    disableCameraContentProtection();
    hideCameraPreview();
    hideRecordingControl();
  }

  if (currentPath) {
    await deleteVideo(currentPath, { showNotification: false });
  }

  const outputPath = createRecordingProject();

  lastRecordingConfig = {
    ...config,
    outputPath,
  };

  if (config.cameraEnabled) {
    const appConfig = getConfig();
    const cameraSettings = appConfig.recording.camera;
    if (cameraSettings) {
      showCameraPreview({
        ...cameraSettings,
        enabled: true,
        selectedDeviceId: config.cameraDeviceId ?? null,
        selectedDeviceName: config.cameraDeviceName ?? null,
      });
    }
    enableCameraContentProtection();
  }

  try {
    await startRecordingWithConfig(
      {
        ...config,
        outputPath,
      },
      showRecordingControl
    );
    console.log('Recording restarted with same area');
  } catch (error) {
    console.error('Error restarting recording:', error);
    lastRecordingConfig = null;
    if (error instanceof Error) {
      await showRecordingError(error);
    }
  }
}

export async function recordScreen(): Promise<void> {
  if (isRecording()) {
    console.log('Already recording');
    return;
  }

  clearSelectedIOSDevice();

  prewarmRecordingControlWindow();
  prewarmRecorder();
  prewarmOverlay();

  try {
    await startAreaSelection({
      mode: 'display',
      onSelected: selection => {
        const bounds =
          selection.x !== undefined &&
          selection.y !== undefined &&
          selection.width !== undefined &&
          selection.height !== undefined
            ? {
                x: selection.x,
                y: selection.y,
                width: selection.width,
                height: selection.height,
              }
            : undefined;
        showPreRecordingControl(bounds);
      },
      onUpdate: selection => {
        if (
          selection.status === 'updated' &&
          selection.x !== undefined &&
          selection.y !== undefined &&
          selection.width !== undefined &&
          selection.height !== undefined
        ) {
          updateRecordingControlPosition({
            x: selection.x,
            y: selection.y,
            width: selection.width,
            height: selection.height,
          });
        }
      },
      onCancelled: () => {
        hidePreRecordingControl();
      },
    });
  } catch (error) {
    console.error('Error starting full screen selection:', error);
  }
}

export async function recordWindow(): Promise<void> {
  if (isRecording()) {
    console.log('Already recording');
    return;
  }

  clearSelectedIOSDevice();

  prewarmRecordingControlWindow();
  prewarmRecorder();
  prewarmOverlay();

  try {
    await startAreaSelection({
      mode: 'window',
      onSelected: selection => {
        const bounds =
          selection.x !== undefined &&
          selection.y !== undefined &&
          selection.width !== undefined &&
          selection.height !== undefined
            ? {
                x: selection.x,
                y: selection.y,
                width: selection.width,
                height: selection.height,
              }
            : undefined;
        showPreRecordingControl(bounds);
      },
      onUpdate: selection => {
        if (
          selection.status === 'updated' &&
          selection.x !== undefined &&
          selection.y !== undefined &&
          selection.width !== undefined &&
          selection.height !== undefined
        ) {
          updateRecordingControlPosition({
            x: selection.x,
            y: selection.y,
            width: selection.width,
            height: selection.height,
          });
        }
      },
      onCancelled: () => {
        hidePreRecordingControl();
      },
    });
  } catch (error) {
    console.error('Error starting window selection:', error);
  }
}
