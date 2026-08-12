export { isRecording, quitRecorder } from './recorder.ts';
export { killAreaSelector } from '@/main/capture/area-selector';
export {
  stopRecordingAction,
  recordArea,
  recordScreen,
  recordWindow,
} from './recording-actions.ts';

import { registerRecordingIpcHandlers } from './recording-ipc.ts';
import { registerCameraPreviewIpcHandlers } from './camera-preview.ts';

registerRecordingIpcHandlers();
registerCameraPreviewIpcHandlers();

export { recordArea as default } from './recording-actions.ts';
