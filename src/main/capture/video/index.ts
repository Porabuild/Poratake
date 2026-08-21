export { isRecording, quitRecorder } from './recorder.ts';
export { killAreaSelector } from '@/main/capture/area-selector';
import {
  stopRecordingAction,
  recordArea,
  recordScreen,
  recordWindow,
} from './recording-actions.ts';

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
  registerRecordingIpcHandlers();
  registerCameraPreviewIpcHandlers();
  initVideoEditor();
  setRecordingTrayStopHandler(stopRecordingAction);
}

export { stopRecordingAction, recordArea, recordScreen, recordWindow };
export { recordArea as default };
