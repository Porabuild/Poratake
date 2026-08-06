import { registerIpcHandlers as registerPinIpcHandlers } from './pin.ts';
import { registerIpcHandlers as registerScreenshotIpcHandlers } from './screenshot.ts';
import { registerCapturePreviewIpc } from '@/main/capture/capture-preview';

export {
  openScreenshotEditor,
  getImageDimensions,
  openImageInEditor,
  openClipboardInEditor,
} from './open-editor.ts';
export { captureArea } from './capture-area.ts';
export { openScreenshotFromHistory } from './open-from-history.ts';
export { default, type CaptureMode } from './screenshot.ts';

registerPinIpcHandlers();
registerScreenshotIpcHandlers();
registerCapturePreviewIpc();
