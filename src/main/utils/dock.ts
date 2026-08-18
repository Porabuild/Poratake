import type { BrowserWindow } from 'electron';
import { app } from 'electron';

type DockWindowType = 'video-editor' | 'screenshot' | 'settings' | 'pin';

const dockWindows = new Map<number, DockWindowType>();
let pendingShow: Promise<void> | null = null;

async function hideDock(): Promise<void> {
  if (process.platform !== 'darwin') {
    return;
  }

  if (pendingShow) {
    await pendingShow;
  }

  if (dockWindows.size > 0) {
    return;
  }

  app.hide();
  app.dock?.hide();
}

export function initDock(): void {
  if (process.platform !== 'darwin') {
    return;
  }
  app.dock?.hide();
}

export async function registerDockWindow(
  window: BrowserWindow,
  type: DockWindowType
): Promise<void> {
  if (process.platform !== 'darwin') {
    return;
  }

  const id = window.id;
  const isFirstWindow = dockWindows.size === 0;
  dockWindows.set(id, type);

  window.on('closed', () => {
    dockWindows.delete(id);
    void hideDock();
  });

  if (isFirstWindow) {
    pendingShow = app.dock?.show() ?? Promise.resolve();
    try {
      await pendingShow;
    } finally {
      pendingShow = null;
    }
  }
}
