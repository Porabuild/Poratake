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
import {
  prepareCapturePreview,
  prewarmCapturePreview,
  showCapturePreview,
} from '@/main/capture/capture-preview';
import type {
  CapturePreviewHandle,
  CapturePreviewPreparation,
} from '@/main/capture/capture-preview';
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
let pendingStartAction: Promise<void> | null = null;
let pendingRecordingAction: {
  type: 'stop' | 'delete' | 'restart';
  promise: Promise<unknown>;
} | null = null;

function isRecordingStartCancellation(error: unknown): boolean {
  return error instanceof Error && error.name === 'AbortError';
}

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

async function handleTerminalRecordingFailure(
  error: Error,
  outputPath: string | null
): Promise<void> {
  currentRecordingType = undefined;
  lastRecordingConfig = null;
  await Promise.allSettled([
    disableCameraContentProtection(),
    hideRecordingControl(),
    outputPath
      ? deleteVideo(outputPath, {
          showNotification: false,
          showErrorDialog: false,
        })
      : Promise.resolve(),
  ]);
  hideCameraPreview();
  await showRecordingError(error);
}

async function stopAndFinalizeRecording(): Promise<string | null> {
  if (pendingStartAction) {
    await pendingStartAction;
  }

  let recordingResult: CompletedRecording | null = null;
  const recordingType = currentRecordingType;

  try {
    recordingResult = await stopRecording(hideRecordingControl);
  } finally {
    currentRecordingType = undefined;
    await disableCameraContentProtection();
    hideCameraPreview();
    await hideRecordingControl();
  }

  if (recordingResult) {
    const editorStatePromise = generateInitialEditorState({
      projectPath: recordingResult.outputPath,
      recordingType,
    });

    const config = getConfig();
    let startHistoryPersistence: () => void = () => {};
    const historyStart = new Promise<void>(resolve => {
      startHistoryPersistence = resolve;
    });
    const historyItemPromise = historyStart.then(() =>
      addToHistory(
        recordingResult.outputPath,
        'video',
        recordingResult.duration
      )
    );
    const historyIdPromise = historyItemPromise.then(item => item?.id);

    let preview: CapturePreviewHandle | null = null;
    let previewPreparation: CapturePreviewPreparation | null = null;

    if (config.recording.showPreview) {
      try {
        previewPreparation = prepareCapturePreview();
        preview = showCapturePreview(
          recordingResult.outputPath,
          'video',
          undefined,
          previewPreparation,
          historyIdPromise,
          editorStatePromise
        );
      } catch (error) {
        console.error('Failed to show recording preview:', error);
        previewPreparation?.dispose();
      }
    }

    if (preview) {
      void preview.revealed.then(startHistoryPersistence);
    } else {
      await editorStatePromise;
      createVideoEditorWindow(recordingResult.outputPath);
      startHistoryPersistence();
    }

    await Promise.all([editorStatePromise, historyItemPromise]);
  }

  return recordingResult?.outputPath ?? null;
}

export function stopRecordingAction(): Promise<string | null> {
  return runRecordingAction('stop', stopAndFinalizeRecording, null);
}

function runRecordingAction<T>(
  type: 'stop' | 'delete' | 'restart',
  action: () => Promise<T>,
  busyResult: T
): Promise<T> {
  if (pendingRecordingAction) {
    if (pendingRecordingAction.type === type) {
      return pendingRecordingAction.promise as Promise<T>;
    }
    return Promise.resolve(busyResult);
  }

  const promise = action();
  pendingRecordingAction = { type, promise };

  return promise.finally(() => {
    if (pendingRecordingAction?.promise === promise) {
      pendingRecordingAction = null;
    }
  });
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

async function startPendingRecordingInternal(
  options: RecordingOptions = {}
): Promise<void> {
  const iosDeviceId = options.iosDeviceId ?? null;
  const iosDeviceName = options.iosDeviceName ?? null;
  const isIOSRecording = iosDeviceId !== null;
  const micEnabled = options.micEnabled ?? false;
  const recordingType = isIOSRecording ? 'ios-device' : undefined;

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

  const includeAudio = options.systemAudio ?? true;
  const micDeviceId = options.micDeviceId ?? null;
  const micDeviceName = options.micDeviceName ?? null;
  const cameraEnabled = options.cameraEnabled ?? false;
  const cameraDeviceId = options.cameraDeviceId ?? null;
  const cameraDeviceName = options.cameraDeviceName ?? null;
  const keyboardEnabled = isIOSRecording
    ? false
    : (options.keyboardEnabled ?? false);

  prewarmCapturePreview();

  let outputPath: string | null = null;
  try {
    outputPath = createRecordingProject();
    const recordingConfig: RecordingConfig = {
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

    if (cameraEnabled) {
      await enableCameraContentProtection();
    }

    await startRecordingWithConfig(
      recordingConfig,
      showRecordingControl,
      hideRecordingControl,
      handleTerminalRecordingFailure
    );
    lastRecordingConfig = recordingConfig;
    currentRecordingType = recordingType;

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
    lastRecordingConfig = null;
    currentRecordingType = undefined;
    if (outputPath) {
      await deleteVideo(outputPath, {
        showNotification: false,
        showErrorDialog: false,
      });
    }
    await hideRecordingControl();
    await disableCameraContentProtection();
    hideCameraPreview();
    if (error instanceof Error && !isRecordingStartCancellation(error)) {
      await showRecordingError(error);
    }
  }
}

export function startPendingRecording(
  options: RecordingOptions = {}
): Promise<void> {
  if (pendingStartAction) {
    return pendingStartAction;
  }

  if (pendingRecordingAction?.type === 'restart') {
    return pendingRecordingAction.promise.then(() => undefined);
  }

  const action = pendingRecordingAction
    ? pendingRecordingAction.promise.then(() =>
        startPendingRecordingInternal(options)
      )
    : startPendingRecordingInternal(options);
  pendingStartAction = action;

  return action.finally(() => {
    if (pendingStartAction === action) {
      pendingStartAction = null;
    }
  });
}

export async function cancelPendingRecording(): Promise<void> {
  cancelAreaSelection();
  hidePreRecordingControl();
}

async function deleteActiveRecording(): Promise<void> {
  if (pendingStartAction) {
    await pendingStartAction;
  }

  const currentPath = getCurrentRecordingPath();
  let stopError: unknown;

  try {
    await stopRecording(hideRecordingControl);
  } catch (error) {
    stopError = error;
  } finally {
    currentRecordingType = undefined;
    await disableCameraContentProtection();
    hideCameraPreview();
    await hideRecordingControl();
  }

  if (currentPath) {
    await deleteVideo(currentPath, { showNotification: true });
  }

  if (stopError) {
    throw stopError;
  }
}

export function deleteRecordingAction(): Promise<void> {
  return runRecordingAction('delete', deleteActiveRecording, undefined);
}

async function restartActiveRecording(): Promise<void> {
  if (pendingStartAction) {
    await pendingStartAction;
  }

  if (!lastRecordingConfig) {
    console.error('No previous recording config to restart');
    return;
  }

  const config = { ...lastRecordingConfig };
  const recordingType = currentRecordingType;
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

  let outputPath: string | null = null;
  try {
    outputPath = createRecordingProject();
    const recordingConfig: RecordingConfig = {
      ...config,
      outputPath,
    };

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
      recordingConfig,
      showRecordingControl,
      hideRecordingControl,
      handleTerminalRecordingFailure
    );
    lastRecordingConfig = recordingConfig;
    currentRecordingType = recordingType;
    console.log('Recording restarted with same area');
  } catch (error) {
    console.error('Error restarting recording:', error);
    if (outputPath) {
      await deleteVideo(outputPath, {
        showNotification: false,
        showErrorDialog: false,
      });
    }
    lastRecordingConfig = null;
    await disableCameraContentProtection();
    hideCameraPreview();
    await hideRecordingControl();
    if (error instanceof Error && !isRecordingStartCancellation(error)) {
      await showRecordingError(error);
    }
  }
}

export function restartRecordingAction(): Promise<void> {
  return runRecordingAction('restart', restartActiveRecording, undefined);
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
