import { execFile } from 'child_process';
import { isMac } from '@/main/utils/platform';
import { prewarmAreaOverlay } from '@/main/capture/area-overlay';
import { prewarmFreezeScreen } from '@/main/capture/freeze-screen';
import { init as initVideoCapture } from '@/main/capture/video';

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
  initVideoCapture();
  prewarmAreaOverlay();
  prewarmFreezeScreen();
  // Daemon cleanup is automatic - child process dies with parent
}
