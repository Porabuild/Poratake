import {
  createVideoEditorWindow,
  getVideoEditorWindow,
  openVideoInEditor,
} from './window-manager';
import { registerAllVideoEditorHandlers } from './ipc';

export type { VideoMetadata } from '@/types/video';
export type { VideoEditorWindowData } from './window-manager';

export function initVideoEditor(): void {
  registerAllVideoEditorHandlers();
}

export { createVideoEditorWindow, getVideoEditorWindow, openVideoInEditor };
