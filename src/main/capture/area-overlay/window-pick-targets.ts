import { screen } from 'electron';
import { listWindows } from '@/main/capture/window-selector';
import { isWindows } from '@/main/utils/platform';
import { getOverlayWindowIds } from './window-pool';
import type { OverlayPickTarget } from './session';

export interface WindowPickTargets {
  targets: OverlayPickTarget[];
  names: Map<number, string>;
  captureRects: Map<number, Electron.Rectangle>;
  prompt: string;
}

export async function resolveWindowPickTargets(): Promise<WindowPickTargets | null> {
  const overlayWindowIds = getOverlayWindowIds();
  const windows = (await listWindows()).filter(
    window => !overlayWindowIds.has(window.windowId)
  );

  if (windows.length === 0) {
    console.error('Window selection failed: no visible windows found');
    return null;
  }

  return {
    targets: windows.map(window => ({
      id: window.windowId,
      rect: isWindows
        ? screen.screenToDipRect(null, window.bounds)
        : window.bounds,
    })),
    names: new Map(
      windows.map(window => [window.windowId, window.title || window.ownerName])
    ),
    captureRects: new Map(
      windows.map(window => [window.windowId, window.bounds])
    ),
    prompt: 'Click a window to select it · Esc to cancel',
  };
}
