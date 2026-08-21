export { isRecording, quitRecorder } from './recorder.ts';
export { killAreaSelector } from '@/main/capture/area-selector';
import {
  cancelPendingRecording,
  deleteRecordingAction,
  startPendingRecording,
  stopRecordingAction,
  recordArea,
  recordScreen,
  recordWindow,
} from './recording-actions.ts';
import { setRecordingControlActions } from './recording-control.ts';

import { registerRecordingIpcHandlers } from './recording-ipc.ts';
import { registerCameraPreviewIpcHandlers } from './camera-preview.ts';
import { initVideoEditor } from './video-editor.ts';
import { setRecordingTrayStopHandler } from '@/main/menu/recording-tray.ts';

let initialized = false;

export function init(): void {
  if (initialized) {
    return;
  }
  initialized = true;
  setRecordingControlActions({
    startPendingRecording,
    cancelPendingRecording,
    stopRecordingAction,
    deleteRecordingAction,
  });
  registerRecordingIpcHandlers();
  registerCameraPreviewIpcHandlers();
  initVideoEditor();
  setRecordingTrayStopHandler(stopRecordingAction);
}

export { stopRecordingAction, recordArea, recordScreen, recordWindow };
export { recordArea as default };
