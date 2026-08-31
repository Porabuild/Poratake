import { screen } from 'electron';
import {
  showRecordingControl,
  hideRecordingControl,
  hidePreRecordingControl,
  detachRecordingControlFromOverlay,
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
  concealAreaSelectorOverlay,
  hasVisibleSelectorOverlay,
  cancelAreaSelection,
  hideAreaSelector,
  killAreaSelector,
  setAreaSelectorFreeze,
} from '@/main/capture/area-selector';
import { isScreenFrozen } from '@/main/capture/freeze-screen';
import {
  isRecording,
  getCurrentRecordingPath,
  startRecordingWithConfig,
  stopRecording,
  createRecordingProject,
  prewarmRecorder,
} from './recorder.ts';
import {
  hideRecordingOverlay,
  prewarmOverlay,
  showRecordedWindowOutline,
} from './overlay.ts';
import {
  startRecordingCountdown,
  MIN_RECORDING_START_DELAY,
  MAX_RECORDING_START_DELAY,
} from './recording-control.ts';
import {
  prepareCapturePreview,
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
import { getConfig, onConfigUpdated, updateConfig } from '@/main/settings';
import { generateInitialEditorState } from './auto-zoom-generator.ts';
import type {
  CompletedRecording,
  RecordingConfig,
  RecordingOptions,
  RecordingType,
} from '@/types/video.ts';
import { isFeatureSupported } from '@/main/system/capabilities';
import { isWindows } from '@/main/utils/platform';

let lastRecordingConfig: RecordingConfig | null = null;
let currentRecordingType: RecordingType | undefined;
let pendingStartAction: Promise<void> | null = null;
let pendingRecordingAction: {
  type: 'stop' | 'delete' | 'restart';
  promise: Promise<unknown>;
} | null = null;
let recordingPreviewPreparation: CapturePreviewPreparation | null = null;

function recordingFrameRate(
  displayId: number | undefined,
  bounds: {
    x: number | undefined;
    y: number | undefined;
    width: number | undefined;
    height: number | undefined;
  }
): number {
  const saved = getConfig().recording.frameRate;
  const configured =
    Number.isFinite(saved) && saved >= 1 && saved <= 240
      ? Math.round(saved)
      : 60;
  const matchingDisplay = screen
    .getAllDisplays()
    .find(candidate => candidate.id === displayId);
  const display =
    matchingDisplay ??
    (bounds.x !== undefined &&
    bounds.y !== undefined &&
    bounds.width !== undefined &&
    bounds.height !== undefined
      ? screen.getDisplayMatching({
          x: bounds.x,
          y: bounds.y,
          width: bounds.width,
          height: bounds.height,
        })
      : undefined);
  if (!display) return configured;
  const refreshRate = Math.round(display.displayFrequency);
  return Math.min(configured, refreshRate > 0 ? refreshRate : configured);
}

function disposeRecordingPreviewPreparation(): void {
  recordingPreviewPreparation?.dispose();
  recordingPreviewPreparation = null;
}

function syncRecordingPreviewPreparation(): void {
  if (!getConfig().recording.showPreview) {
    disposeRecordingPreviewPreparation();
    return;
  }

  if (recordingPreviewPreparation) return;

  try {
    recordingPreviewPreparation = prepareCapturePreview();
  } catch (error) {
    console.error('Failed to prepare recording preview:', error);
  }
}

function prepareRecordingPreview(): void {
  disposeRecordingPreviewPreparation();
  syncRecordingPreviewPreparation();
}

function takeRecordingPreviewPreparation(): CapturePreviewPreparation | null {
  const preparation = recordingPreviewPreparation;
  recordingPreviewPreparation = null;
  return preparation;
}

onConfigUpdated(updates => {
  if (updates.recording?.showPreview === undefined || !isRecording()) {
    return;
  }

  syncRecordingPreviewPreparation();
});

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

async function handleTerminalRecordingFailure(
  error: Error,
  outputPath: string | null
): Promise<void> {
  disposeRecordingPreviewPreparation();
  currentRecordingType = undefined;
  lastRecordingConfig = null;
  concealAreaSelectorOverlay();
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

function stopRecordingWhenTargetClosed(): void {
  void stopRecordingAction();
}

async function teardownActiveRecordingPresentation(): Promise<void> {
  currentRecordingType = undefined;
  concealAreaSelectorOverlay();
  await disableCameraContentProtection();
  hideCameraPreview();
  await hideRecordingControl();
}

async function stopAndFinalizeRecording(): Promise<string | null> {
  if (pendingStartAction) {
    await pendingStartAction;
  }

  let recordingResult: CompletedRecording | null = null;
  const recordingType = currentRecordingType;

  try {
    recordingResult = await stopRecording(hideRecordingControl);
  } catch (error) {
    disposeRecordingPreviewPreparation();
    throw error;
  } finally {
    await teardownActiveRecordingPresentation();
  }

  let previewPreparation = takeRecordingPreviewPreparation();
  if (recordingResult) {
    try {
      const editorStatePromise = generateInitialEditorState({
        projectPath: recordingResult.outputPath,
        recordingType,
        duration: recordingResult.duration,
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

      if (config.recording.showPreview) {
        try {
          previewPreparation ??= prepareCapturePreview();
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
        }
      }

      if (preview) {
        void preview.revealed.then(startHistoryPersistence);
      } else {
        startHistoryPersistence();
      }

      await Promise.all([editorStatePromise, historyItemPromise]);
    } finally {
      previewPreparation?.dispose();
    }
  } else {
    previewPreparation?.dispose();
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
      freeze: false,
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

async function launchRecording(
  config: RecordingConfig,
  recordingType: RecordingType | undefined
): Promise<void> {
  await startRecordingWithConfig(
    config,
    () => showRecordingControl(config),
    hideRecordingControl,
    handleTerminalRecordingFailure,
    isWindows || hasVisibleSelectorOverlay() || config.windowId !== undefined,
    stopRecordingWhenTargetClosed
  );

  lastRecordingConfig = config;
  currentRecordingType = recordingType;
  syncRecordingPreviewPreparation();
}

async function abortRecordingStart(
  outputPath: string | null,
  error: unknown,
  message: string
): Promise<void> {
  disposeRecordingPreviewPreparation();
  console.error(message, error);
  lastRecordingConfig = null;
  currentRecordingType = undefined;
  concealAreaSelectorOverlay();

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
  let windowId: number | undefined;
  let windowName: string | undefined;

  if (!isIOSRecording) {
    detachRecordingControlFromOverlay();
    await setAreaSelectorFreeze(false);
    if (isScreenFrozen()) {
      console.error('Failed to release frozen displays before recording');
      return;
    }
    const selection = await confirmAreaSelection({ keepOverlayVisible: true });

    if (!selection || selection.status === 'cancelled') {
      console.log('No pending selection to record');
      return;
    }

    x = selection.x;
    y = selection.y;
    width = selection.width;
    height = selection.height;
    screenId = selection.screenId;
    windowId = selection.windowId;
    windowName = selection.windowName;

    if (
      x === undefined ||
      y === undefined ||
      width === undefined ||
      height === undefined
    ) {
      concealAreaSelectorOverlay();
      console.error('Invalid area selection');
      return;
    }

    if (windowId !== undefined) {
      concealAreaSelectorOverlay();
    }
  }

  const startDelay = Math.round(getConfig().recording.startDelay ?? 0);
  const clampedDelay = Math.min(
    Math.max(startDelay, MIN_RECORDING_START_DELAY),
    MAX_RECORDING_START_DELAY
  );

  if (!isIOSRecording && clampedDelay === 0) {
    await hidePreRecordingControl(false);
  }

  if (clampedDelay > 0) {
    const countdownResult = await startRecordingCountdown(clampedDelay);
    if (countdownResult === 'cancelled') {
      concealAreaSelectorOverlay();
      await Promise.all([
        hidePreRecordingControl(),
        hideRecordingOverlay(true),
      ]);
      console.log('Recording cancelled during countdown');
      return;
    }

    if (!isIOSRecording) {
      await hidePreRecordingControl(false);
    }
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

  prepareRecordingPreview();

  let outputPath: string | null = null;
  try {
    outputPath = createRecordingProject();
    const recordingConfig: RecordingConfig = {
      x,
      y,
      width,
      height,
      displayId: screenId,
      windowId,
      windowName,
      includeAudio,
      micEnabled,
      micDeviceId,
      micDeviceName,
      cameraEnabled,
      cameraDeviceId,
      cameraDeviceName,
      keyboardEnabled,
      frameRate: recordingFrameRate(screenId, { x, y, width, height }),
      outputPath,
      iosDeviceId,
      iosDeviceName,
    };

    if (cameraEnabled) {
      await enableCameraContentProtection();
    }

    await launchRecording(recordingConfig, recordingType);

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
    await abortRecordingStart(outputPath, error, 'Error starting recording:');
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
  disposeRecordingPreviewPreparation();
  cancelAreaSelection();
  hidePreRecordingControl();
  await hideRecordingOverlay(true);
}

async function deleteActiveRecording(): Promise<void> {
  if (pendingStartAction) {
    await pendingStartAction;
  }

  disposeRecordingPreviewPreparation();

  const currentPath = getCurrentRecordingPath();
  let stopError: unknown;

  try {
    await stopRecording(hideRecordingControl);
  } catch (error) {
    stopError = error;
  } finally {
    await teardownActiveRecordingPresentation();
  }

  if (currentPath) {
    await deleteVideo(currentPath, { showNotification: false });
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
  disposeRecordingPreviewPreparation();

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
    prepareRecordingPreview();
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

    await launchRecording(recordingConfig, recordingType);
    console.log('Recording restarted with same area');
  } catch (error) {
    await abortRecordingStart(outputPath, error, 'Error restarting recording:');
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
      freeze: false,
      visible: false,
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
      freeze: false,
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
        showPreRecordingControl(bounds, selection.windowName);
        if (selection.windowId === undefined) return;

        void hideAreaSelector();
        void showRecordedWindowOutline(selection.windowId);
      },
      onCancelled: () => {
        hidePreRecordingControl();
        void hideRecordingOverlay(true);
      },
    });
  } catch (error) {
    console.error('Error starting window selection:', error);
  }
}
