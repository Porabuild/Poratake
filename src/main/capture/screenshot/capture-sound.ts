import { execFile } from 'child_process';
import { getConfig } from '@/main/settings';
import { isFeatureSupported } from '@/main/system/capabilities';

const CAPTURE_SOUND_PATH = '/System/Library/Sounds/Glass.aiff';

export function playCaptureSound(): void {
  if (!isFeatureSupported('capture-sound')) {
    return;
  }

  if (!getConfig().general?.playSoundOnScreenshot) {
    return;
  }

  execFile('afplay', [CAPTURE_SOUND_PATH], error => {
    if (error) {
      console.error('Failed to play capture sound:', error);
    }
  });
}
