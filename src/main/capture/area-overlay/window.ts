import type { BrowserWindow, Display } from 'electron';
import {
  createClickThroughWindow,
  loadAppWindow,
} from '@/main/utils/window-factory';

export function createOverlayWindow(display: Display): BrowserWindow {
  const overlayWindow = createClickThroughWindow({
    bounds: display.bounds,
    level: 'screen-saver',
    panel: true,
  });

  loadAppWindow(overlayWindow, { type: 'area-overlay' });

  return overlayWindow;
}
