import { daemon } from '@/main/daemon';
import { getAccentColor } from '@/main/settings/accent';
import { isWindows } from '@/main/utils/platform';

let isVisible = false;
let outlinedWindowId: number | null = null;

export async function showRecordedWindowOutline(
  windowId: number
): Promise<void> {
  if (outlinedWindowId === windowId) return;

  try {
    await daemon.call('recording-overlay', 'showWindow', {
      windowId,
      color: getAccentColor(),
    });
    outlinedWindowId = windowId;
    isVisible = true;
  } catch (error) {
    console.error('Failed to outline the recorded window:', error);
  }
}

export async function showRecordingOverlay(
  x: number,
  y: number,
  width: number,
  height: number
): Promise<void> {
  if (isWindows) return;

  try {
    await daemon.call('recording-overlay', 'show', { x, y, width, height });
    isVisible = true;
  } catch (error) {
    console.error('Failed to show recording overlay:', error);
    throw error;
  }
}

export async function hideRecordingOverlay(force = false): Promise<void> {
  outlinedWindowId = null;

  if (!isVisible && !force) {
    return;
  }

  try {
    await daemon.call('recording-overlay', 'hide');
    isVisible = false;
  } catch (error) {
    console.error('Failed to hide recording overlay:', error);
  }
}

export async function prewarmOverlay(): Promise<void> {
  // No longer needed - daemon is always running
}
