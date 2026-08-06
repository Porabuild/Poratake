import {
  hidePreRecordingControl,
  hideRecordingControl,
} from './recording-control.ts';
import { hideCameraPreview } from './camera-preview.ts';
import { cancelAreaSelection } from '@/main/capture/area-selector';
import { hideRecordingOverlay } from './overlay.ts';
import { quitRecorder } from './recorder.ts';

export async function cleanupRecordingUIForMicPermission(): Promise<void> {
  cancelAreaSelection();

  await quitRecorder();

  hidePreRecordingControl();
  hideRecordingControl();
  hideCameraPreview();

  await hideRecordingOverlay();
}
