import { nativeTheme } from 'electron';
import { daemon } from '@/main/daemon';
import { getAccentColor } from '@/main/settings/accent';
import { onConfigUpdated } from '@/main/settings';
import { isWindows } from '@/main/utils/platform';
import { getRecordingControlBrowserWindow } from './recording-control-window';

let isVisible = false;
let outlinedWindowId: number | null = null;
let accentListenersBound = false;

function getControlWindowHandle(): number | null {
  const controlWindow = getRecordingControlBrowserWindow();
  if (!controlWindow || controlWindow.isDestroyed()) {
    return null;
  }
  return Number(controlWindow.getNativeWindowHandle().readBigUInt64LE());
}

async function applyWindowOutline(windowId: number): Promise<boolean> {
  const belowWindowId = isWindows ? getControlWindowHandle() : null;

  try {
    await daemon.call('recording-overlay', 'showWindow', {
      windowId,
      color: getAccentColor(),
      ...(belowWindowId === null ? {} : { belowWindowId }),
    });
    return true;
  } catch (error) {
    console.error('Failed to outline the recorded window:', error);
    return false;
  }
}

function refreshOutlinedWindowAccent(): void {
  if (outlinedWindowId === null) return;
  void applyWindowOutline(outlinedWindowId);
}

function bindAccentListeners(): void {
  if (accentListenersBound) return;
  accentListenersBound = true;

  onConfigUpdated(updates => {
    if (!updates.appearance) return;
    refreshOutlinedWindowAccent();
  });

  nativeTheme.on('updated', refreshOutlinedWindowAccent);
}

export async function showRecordedWindowOutline(
  windowId: number
): Promise<void> {
  if (outlinedWindowId === windowId) return;

  bindAccentListeners();

  if (await applyWindowOutline(windowId)) {
    outlinedWindowId = windowId;
    isVisible = true;
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
