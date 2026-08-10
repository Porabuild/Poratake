import { execFile } from 'child_process';
import { isMac } from '@/main/utils/platform';
import { prewarmCapturePreview } from '@/main/capture/capture-preview';
import { prewarmAreaOverlay } from '@/main/capture/area-overlay';
import { onConfigUpdated } from '@/main/settings';

export function resetScreenCaptureCache(): void {
  if (!isMac) {
    return;
  }

  execFile('killall', ['screencapturemgr'], error => {
    if (error) {
      console.log('screencapturemgr not running or already killed');
    } else {
      console.log(
        'Reset screencapturemgr to clear stale ScreenCaptureKit cache'
      );
    }
  });
}

export function init(): void {
  onConfigUpdated(updates => {
    if (!updates.screenshot && !updates.recording) return;
    prewarmCapturePreview();
  });
  prewarmCapturePreview();
  if (!isMac) {
    prewarmAreaOverlay();
  }
  // Daemon cleanup is automatic - child process dies with parent
}
