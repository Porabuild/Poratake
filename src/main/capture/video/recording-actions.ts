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
import type {
  CompletedRecording,
  RecordingConfig,
  RecordingType,
} from '@/types/video.ts';
import { isFeatureSupported } from '@/main/system/capabilities';

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

async function handleTerminalRecordingFailure(error: Error): Promise<void> {
  currentRecordingType = undefined;
  await Promise.allSettled([
    disableCameraContentProtection(),
    hideRecordingControl(),
  ]);
  hideCameraPreview();
  await showRecordingError(error);
}

export async function stopRecordingAction(): Promise<string | null> {
  let recordingResult: CompletedRecording | null = null;

  try {
    recordingResult = await stopRecording(hideRecordingControl);
  } finally {
    await disableCameraContentProtection();
    hideCameraPreview();
    await hideRecordingControl();
  }

  if (recordingResult) {
    const historyItem = await addToHistory(
      recordingResult.outputPath,
      'video',
      recordingResult.duration
    );
    await generateInitialEditorState({
      projectPath: recordingResult.outputPath,
      recordingType: currentRecordingType,
    });

    const config = getConfig();
    if (config.recording.showPreview) {
      showCapturePreview(recordingResult.outputPath, 'video', historyItem?.id);
    } else {
      createVideoEditorWindow(recordingResult.outputPath);
    }
  }

  currentRecordingType = undefined;

  return recordingResult?.outputPath ?? null;
}

export async function recordArea(): Promise<void> {
  if (!isFeatureSupported('recording')) {
    return;
  }

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
  const micEnabled = options.micEnabled ?? false;

  currentRecordingType = isIOSRecording ? 'ios-device' : undefined;

  if (micEnabled) {
    const granted = await checkAndRequestMicrophonePermission();
    if (!granted) {
      return;
    }
  }

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
    if (cameraEnabled) {
      await enableCameraContentProtection();
    }

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
      showRecordingControl,
      hideRecordingControl,
      handleTerminalRecordingFailure
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
    await disableCameraContentProtection();
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
    await disableCameraContentProtection();
    hideCameraPreview();
    await hideRecordingControl();
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
    await disableCameraContentProtection();
    hideCameraPreview();
    await hideRecordingControl();
  }

  if (currentPath) {
    await deleteVideo(currentPath, { showNotification: false });
  }

  const outputPath = createRecordingProject();

  lastRecordingConfig = {
    ...config,
    outputPath,
  };

  try {
    if (config.cameraEnabled) {
      const appConfig = getConfig();
      const cameraSettings = appConfig.recording.camera;
      if (cameraSettings) {
        await showCameraPreview({
          ...cameraSettings,
          enabled: true,
          selectedDeviceId: config.cameraDeviceId ?? null,
          selectedDeviceName: config.cameraDeviceName ?? null,
        });
      }
      await enableCameraContentProtection();
    }

    await startRecordingWithConfig(
      {
        ...config,
        outputPath,
      },
      showRecordingControl,
      hideRecordingControl,
      handleTerminalRecordingFailure
    );
    console.log('Recording restarted with same area');
  } catch (error) {
    console.error('Error restarting recording:', error);
    lastRecordingConfig = null;
    await disableCameraContentProtection();
    hideCameraPreview();
    await hideRecordingControl();
    if (error instanceof Error) {
      await showRecordingError(error);
    }
  }
}

export async function recordScreen(): Promise<void> {
  if (!isFeatureSupported('recording')) {
    return;
  }

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
  if (!isFeatureSupported('recording')) {
    return;
  }

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
