import { exec } from 'child_process';
import { isMac } from '@/main/utils/platform';

export function resetScreenCaptureCache(): void {
  if (!isMac) {
    return;
  }

  exec('killall screencapturemgr 2>/dev/null', error => {
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
  // Daemon cleanup is automatic - child process dies with parent
}
