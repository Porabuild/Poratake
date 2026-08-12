import { app, screen } from 'electron';
import path from 'path';
import { existsSync } from 'fs';
import {
  hideRecordingOverlay,
  showRecordedWindowOutline,
  showRecordingOverlay,
} from './overlay.ts';
import {
  showRecordingTray,
  hideRecordingTray,
} from '@/main/menu/recording-tray.ts';
import { ensureDirectoryExists, isValidDirectory } from '@/main/utils/paths.ts';
import { getConfig } from '@/main/settings';
import { generateFilename } from '@/main/utils/filename-generator';
import { DEFAULT_STORAGE_CONFIG } from '@/types/settings';
import { createProjectFolder } from './recording-project';
import {
  PROJECT_EXTENSION,
  type CompletedRecording,
  type RecorderResponse,
  type RecorderState,
  type RecordingConfig,
} from '@/types/video';
import { daemon } from '@/main/daemon';
import { isWindows } from '@/main/utils/platform';

const RECORDING_TARGET_CLOSED = 'TARGET_CLOSED';

let recorderState: RecorderState = 'idle';
let currentRecordingPath: string | null = null;
let currentDuration = 0;
let recordingGeneration = 0;
let isStarting = false;
let pendingStart: Promise<void> | null = null;
let pendingStop: Promise<CompletedRecording | null> | null = null;
let pendingFailureCleanup: Promise<void> | null = null;
let activeRecordingErrorListener: {
  generation: number;
  handler: (event: string, data: unknown) => void;
} | null = null;

type RecordingFailureHandler = (
  error: Error,
  outputPath: string | null
) => void | Promise<void>;

function throwIfRecordingStartCancelled(generation: number): void {
  if (generation === recordingGeneration) return;

  const error = new Error('Recording start cancelled');
  error.name = 'AbortError';
  throw error;
}

function clearRecordingErrorListener(generation?: number): void {
  if (
    !activeRecordingErrorListener ||
    (generation !== undefined &&
      activeRecordingErrorListener.generation !== generation)
  ) {
    return;
  }

  daemon.offEvent(activeRecordingErrorListener.handler);
  activeRecordingErrorListener = null;
}

async function handleTerminalRecordingError(
  generation: number,
  error: Error,
  onFailure?: RecordingFailureHandler
): Promise<void> {
  if (activeRecordingErrorListener?.generation !== generation) {
    return;
  }

  clearRecordingErrorListener(generation);
  recordingGeneration += 1;
  const outputPath = currentRecordingPath;
  recorderState = 'idle';
  currentRecordingPath = null;
  currentDuration = 0;
  hideRecordingTray();
  await hideRecordingOverlay(true);

  try {
    await onFailure?.(error, outputPath);
  } catch (failureError) {
    console.error('Failed to handle recording error:', failureError);
  }
}

export function getRecordingsDir(): string {
  const config = getConfig();
  const customPath = config.storage?.recordingsPath;

  if (customPath && isValidDirectory(customPath)) {
    return ensureDirectoryExists(customPath);
  }

  const moviesPath = app.getPath('videos');
  const defaultDir = path.join(moviesPath, 'Poratake');
  return ensureDirectoryExists(defaultDir);
}

export function generateRecordingProjectName(): string {
  const config = getConfig();
  const pattern =
    config.storage?.namingPattern || DEFAULT_STORAGE_CONFIG.namingPattern;

  const baseName = generateFilename({
    pattern,
    type: 'Recording',
    extension: '',
  });

  return baseName.replace(/\.$/, '') + PROJECT_EXTENSION;
}

export function createRecordingProject(): string {
  const recordingsDir = getRecordingsDir();
  const generatedName = generateRecordingProjectName();
  const { name, ext } = path.parse(generatedName);
  let projectPath = path.join(recordingsDir, generatedName);
  let suffix = 2;

  while (existsSync(projectPath)) {
    projectPath = path.join(recordingsDir, `${name} ${suffix}${ext}`);
    suffix++;
  }

  return createProjectFolder(projectPath);
}

export function generateRecordingExportName(extension = 'mp4'): string {
  const config = getConfig();
  const pattern =
    config.storage?.namingPattern || DEFAULT_STORAGE_CONFIG.namingPattern;

  return generateFilename({
    pattern,
    type: 'Recording',
    extension,
  });
}

export function isRecording(): boolean {
  return recorderState === 'recording' || recorderState === 'paused';
}

export function isPaused(): boolean {
  return recorderState === 'paused';
}

export function getRecordingDuration(): number {
  return currentDuration;
}

export function getRecordingState(): RecorderState {
  return recorderState;
}

export function getCurrentRecordingPath(): string | null {
  return currentRecordingPath;
}

async function startRecording(
  config: RecordingConfig,
  showControl: () => void | Promise<void>,
  hideControl?: () => void | Promise<void>,
  onFailure?: RecordingFailureHandler,
  skipRecordingOverlay = false,
  onTargetClosed?: () => void
): Promise<void> {
  if (pendingFailureCleanup) {
    await pendingFailureCleanup;
  }

  if (isStarting || recorderState !== 'idle') {
    throw new Error('A recording is already active');
  }

  const isAreaRecording =
    config.x !== undefined &&
    config.y !== undefined &&
    config.width !== undefined &&
    config.height !== undefined;

  const isIOSRecording = config.iosDeviceId != null;
  const recordingBounds = isAreaRecording
    ? {
        x: config.x!,
        y: config.y!,
        width: config.width!,
        height: config.height!,
      }
    : undefined;
  const nativeBounds =
    recordingBounds && isWindows
      ? screen.dipToScreenRect(null, recordingBounds)
      : recordingBounds;

  clearRecordingErrorListener();
  const generation = ++recordingGeneration;
  let startupSettled = false;
  let rejectTerminalError: (error: Error) => void = () => {};
  const terminalError = new Promise<never>((_, reject) => {
    rejectTerminalError = reject;
  });
  const errorHandler = (event: string, data: unknown) => {
    if (
      event !== 'screen-recorder:error' ||
      activeRecordingErrorListener?.generation !== generation
    ) {
      return;
    }

    const details = data as { code?: unknown; message?: unknown } | undefined;
    const error = new Error(
      typeof details?.message === 'string'
        ? details.message
        : 'Recording failed'
    ) as Error & { code?: string };
    if (typeof details?.code === 'string') {
      error.code = details.code;
    }

    if (!startupSettled) {
      rejectTerminalError(error);
      return;
    }

    if (error.code === RECORDING_TARGET_CLOSED && onTargetClosed) {
      clearRecordingErrorListener(generation);
      onTargetClosed();
      return;
    }

    const cleanup = handleTerminalRecordingError(generation, error, onFailure);
    pendingFailureCleanup = cleanup;
    const clearPendingCleanup = () => {
      if (pendingFailureCleanup === cleanup) {
        pendingFailureCleanup = null;
      }
    };
    void cleanup.then(clearPendingCleanup, clearPendingCleanup);
  };
  activeRecordingErrorListener = { generation, handler: errorHandler };
  daemon.onEvent(errorHandler);
  let pendingStartup: Promise<unknown> | null = null;
  let pendingControl: Promise<void> | null = null;

  isStarting = true;
  try {
    const startup = Promise.allSettled([
      Promise.resolve().then(() =>
        daemon.call<RecorderResponse>(
          'screen-recorder',
          'start',
          {
            x: nativeBounds?.x ?? config.x,
            y: nativeBounds?.y ?? config.y,
            width: nativeBounds?.width ?? config.width,
            height: nativeBounds?.height ?? config.height,
            displayId: config.displayId,
            windowId: config.windowId,
            includeAudio: config.includeAudio ?? true,
            micEnabled: config.micEnabled ?? false,
            micDeviceId: config.micDeviceId,
            micDeviceName: config.micDeviceName,
            cameraEnabled: config.cameraEnabled ?? false,
            cameraDeviceId: config.cameraDeviceId,
            cameraDeviceName: config.cameraDeviceName,
            keyboardEnabled: config.keyboardEnabled ?? false,
            frameRate: config.frameRate ?? 60,
            outputPath: config.outputPath,
            iosDeviceId: config.iosDeviceId,
            iosDeviceName: config.iosDeviceName,
          },
          60000
        )
      ),
      Promise.resolve().then(() => {
        if (config.windowId !== undefined) {
          return showRecordedWindowOutline(config.windowId);
        }
        if (!nativeBounds || isIOSRecording || skipRecordingOverlay) {
          return undefined;
        }
        return showRecordingOverlay(
          nativeBounds.x,
          nativeBounds.y,
          nativeBounds.width,
          nativeBounds.height
        );
      }),
    ] as const);
    pendingStartup = startup;
    const [startResult, overlayResult] = await Promise.race([
      startup,
      terminalError,
    ]);
    throwIfRecordingStartCancelled(generation);

    if (startResult.status === 'rejected') {
      throw startResult.reason;
    }
    if (!startResult.value.success) {
      throw new Error(startResult.value.message || 'Failed to start recording');
    }
    if (overlayResult.status === 'rejected') {
      throw overlayResult.reason;
    }

    pendingControl = Promise.resolve().then(showControl);
    await Promise.race([pendingControl, terminalError]);
    throwIfRecordingStartCancelled(generation);
    recorderState = 'recording';
    currentRecordingPath = config.outputPath;
    showRecordingTray();
    startupSettled = true;
  } catch (error) {
    clearRecordingErrorListener(generation);
    await Promise.allSettled([
      pendingStartup ?? Promise.resolve(),
      pendingControl ?? Promise.resolve(),
    ]);
    await Promise.allSettled([
      daemon.call('screen-recorder', 'stop', undefined, 60000),
      hideRecordingOverlay(true),
      hideControl ? Promise.resolve().then(hideControl) : Promise.resolve(),
    ]);
    hideRecordingTray();
    recorderState = 'idle';
    currentRecordingPath = null;
    currentDuration = 0;
    throw error;
  } finally {
    isStarting = false;
  }
}

export function startRecordingWithConfig(
  config: RecordingConfig,
  showControl: () => void | Promise<void>,
  hideControl?: () => void | Promise<void>,
  onFailure?: RecordingFailureHandler,
  skipRecordingOverlay = false,
  onTargetClosed?: () => void
): Promise<void> {
  if (pendingStart) {
    return Promise.reject(new Error('A recording is already active'));
  }

  const start = startRecording(
    config,
    showControl,
    hideControl,
    onFailure,
    skipRecordingOverlay,
    onTargetClosed
  );
  pendingStart = start;

  return start.finally(() => {
    if (pendingStart === start) {
      pendingStart = null;
    }
  });
}

export async function pauseRecording(): Promise<void> {
  if (recorderState !== 'recording') {
    console.log('Not recording, cannot pause');
    return;
  }

  const generation = recordingGeneration;
  const response = await daemon.call<RecorderResponse>(
    'screen-recorder',
    'pause'
  );

  if (generation !== recordingGeneration || recorderState !== 'recording') {
    return;
  }

  if (!response.success) {
    throw new Error(response.message || 'Failed to pause recording');
  }

  recorderState = 'paused';
  if (response.duration !== undefined) {
    currentDuration = response.duration;
  }
}

export async function resumeRecording(): Promise<void> {
  if (recorderState !== 'paused') {
    console.log('Not paused, cannot resume');
    return;
  }

  const generation = recordingGeneration;
  const response = await daemon.call<RecorderResponse>(
    'screen-recorder',
    'resume'
  );

  if (generation !== recordingGeneration || recorderState !== 'paused') {
    return;
  }

  if (!response.success) {
    throw new Error(response.message || 'Failed to resume recording');
  }

  recorderState = 'recording';
  if (response.duration !== undefined) {
    currentDuration = response.duration;
  }
}

async function stopActiveRecording(
  hideControl: () => void | Promise<void>
): Promise<CompletedRecording | null> {
  const outputPath = currentRecordingPath;
  const fallbackDuration = currentDuration;
  clearRecordingErrorListener();
  recordingGeneration += 1;

  try {
    await hideControl();
  } catch (error) {
    console.error('Failed to hide recording controls:', error);
  }

  let response: RecorderResponse | null = null;
  let stopError: Error | null = null;

  try {
    response = await daemon.call<RecorderResponse>(
      'screen-recorder',
      'stop',
      undefined,
      60000
    );
    if (!response.success) {
      stopError = new Error(response.message || 'Failed to stop recording');
    }
  } catch (error) {
    stopError = error instanceof Error ? error : new Error(String(error));
  }

  await hideRecordingOverlay();
  hideRecordingTray();

  currentRecordingPath = null;
  currentDuration = 0;
  recorderState = 'idle';

  if (stopError) {
    throw stopError;
  }

  const finalPath = response?.outputPath || outputPath;
  if (!finalPath) {
    return null;
  }

  console.log('Recording saved to:', finalPath);

  return {
    outputPath: finalPath,
    cursorPath: response?.cursorPath,
    cameraPath: response?.cameraPath,
    keysPath: response?.keysPath,
    systemAudioPath: response?.systemAudioPath,
    micAudioPath: response?.micAudioPath,
    duration: response?.duration ?? fallbackDuration,
  };
}

export function stopRecording(
  hideControl: () => void | Promise<void>
): Promise<CompletedRecording | null> {
  if (pendingStop) {
    return pendingStop;
  }

  if (!isRecording()) {
    console.log('Not recording, nothing to stop');
    return Promise.resolve(null);
  }

  const stop = stopActiveRecording(hideControl);
  pendingStop = stop;

  return stop.finally(() => {
    if (pendingStop === stop) {
      pendingStop = null;
    }
  });
}

export async function quitRecorder(): Promise<void> {
  recordingGeneration += 1;
  clearRecordingErrorListener();
  await pendingStart?.catch(() => {});
  await hideRecordingOverlay();
  hideRecordingTray();

  if (pendingStop) {
    await pendingStop.catch(() => {});
  } else if (recorderState !== 'idle') {
    try {
      await daemon.call('screen-recorder', 'stop', undefined, 5000);
    } catch {
      // Ignore errors during quit
    }
  }

  recorderState = 'idle';
  currentRecordingPath = null;
  currentDuration = 0;
}

export async function prewarmRecorder(): Promise<void> {
  try {
    await daemon.call('screen-recorder', 'status', undefined, 5000);
  } catch (error) {
    console.error('Failed to prewarm recorder:', error);
  }
}
