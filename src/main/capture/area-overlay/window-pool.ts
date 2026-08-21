import { app, screen } from 'electron';
import type { BrowserWindow, Display } from 'electron';
import { daemon } from '@/main/daemon';
import { isWindows } from '@/main/utils/platform';
import { createOverlayWindow } from './window';

export interface PooledOverlayWindow {
  window: BrowserWindow;
  webContentsId: number;
  displayId: number;
  prepared: boolean;
  initialized: Promise<void>;
}

export type OverlayWindowLostHandler = (pooled: PooledOverlayWindow) => void;

const pooledWindowsByDisplay = new Map<number, PooledOverlayWindow>();
const pooledWindowsByWebContents = new Map<number, PooledOverlayWindow>();
const overlayVisibilityVersions = new WeakMap<BrowserWindow, number>();

let windowLostHandler: OverlayWindowLostHandler | null = null;

export function setOverlayWindowLostHandler(
  handler: OverlayWindowLostHandler
): void {
  windowLostHandler = handler;
}

export function nativeWindowId(window: BrowserWindow): number {
  return Number(window.getNativeWindowHandle().readBigUInt64LE());
}

export function nativeWindowHandle(window: BrowserWindow): string {
  return nativeWindowId(window).toString();
}

export function getOverlayWindowIds(): Set<number> {
  const ids = new Set<number>();

  for (const pooled of pooledWindowsByDisplay.values()) {
    if (pooled.window.isDestroyed()) continue;
    ids.add(nativeWindowId(pooled.window));
  }

  return ids;
}

export function pooledWindowForWebContents(
  webContentsId: number
): PooledOverlayWindow | undefined {
  return pooledWindowsByWebContents.get(webContentsId);
}

export function nextOverlayVisibilityVersion(window: BrowserWindow): number {
  const version = (overlayVisibilityVersions.get(window) ?? 0) + 1;
  overlayVisibilityVersions.set(window, version);
  return version;
}

export function removePooledWindow(entry: PooledOverlayWindow): void {
  if (pooledWindowsByDisplay.get(entry.displayId) === entry) {
    pooledWindowsByDisplay.delete(entry.displayId);
  }
  pooledWindowsByWebContents.delete(entry.webContentsId);
}

export function prepareOverlayWindow(
  window: BrowserWindow,
  method: 'hideWindowWithoutTransitions' | 'showWindowWithoutTransitions'
): Promise<void> {
  if (window.isDestroyed()) return Promise.resolve();

  if (!isWindows) {
    if (method === 'hideWindowWithoutTransitions') {
      window.setOpacity(0);
      return Promise.resolve();
    }

    if (!window.isVisible()) {
      showInactiveWithoutTransitions(window);
    }
    window.setOpacity(1);
    return Promise.resolve();
  }

  return daemon
    .call('area-selector', method, { windowHandle: nativeWindowHandle(window) })
    .then(() => undefined)
    .catch(error => {
      console.error(`Failed to apply overlay window ${method}:`, error);
    });
}

export function showInactiveWithoutTransitions(window: BrowserWindow): void {
  const animationsDisabled = app.commandLine.hasSwitch(
    'wm-window-animations-disabled'
  );
  if (!animationsDisabled) {
    app.commandLine.appendSwitch('wm-window-animations-disabled');
  }
  try {
    window.showInactive();
  } finally {
    if (!animationsDisabled) {
      app.commandLine.removeSwitch('wm-window-animations-disabled');
    }
  }
}

function createPooledWindow(display: Display): PooledOverlayWindow {
  const window = createOverlayWindow(display);
  showInactiveWithoutTransitions(window);
  const initialized = prepareOverlayWindow(
    window,
    'hideWindowWithoutTransitions'
  ).then(() => {
    if (window.isDestroyed() || window.isVisible()) return;
    window.setOpacity(1);
  });
  const pooled: PooledOverlayWindow = {
    window,
    webContentsId: window.webContents.id,
    displayId: display.id,
    prepared: false,
    initialized,
  };
  pooledWindowsByDisplay.set(display.id, pooled);
  pooledWindowsByWebContents.set(pooled.webContentsId, pooled);

  window.webContents.on('render-process-gone', () => {
    removePooledWindow(pooled);
    windowLostHandler?.(pooled);
  });

  window.on('closed', () => {
    removePooledWindow(pooled);
    windowLostHandler?.(pooled);
  });

  return pooled;
}

export async function concealOverlayWindow(
  window: BrowserWindow
): Promise<void> {
  if (window.isDestroyed()) return;
  nextOverlayVisibilityVersion(window);
  window.setIgnoreMouseEvents(true);
  await prepareOverlayWindow(window, 'hideWindowWithoutTransitions');
}

function canActivateOverlayWindow(
  window: BrowserWindow,
  version: number
): boolean {
  return (
    !window.isDestroyed() && overlayVisibilityVersions.get(window) === version
  );
}

export async function showOverlayWindow(window: BrowserWindow): Promise<void> {
  const version = nextOverlayVisibilityVersion(window);

  if (isWindows && window.isVisible()) {
    window.setIgnoreMouseEvents(false);
    return;
  }

  window.setIgnoreMouseEvents(true);
  await prepareOverlayWindow(window, 'showWindowWithoutTransitions');
  if (!canActivateOverlayWindow(window, version)) return;

  window.setIgnoreMouseEvents(false);
}

function getPooledWindow(display: Display): PooledOverlayWindow {
  const existing = pooledWindowsByDisplay.get(display.id);
  const pooled =
    existing && !existing.window.isDestroyed()
      ? existing
      : createPooledWindow(display);

  pooled.window.setBounds(display.bounds);

  return pooled;
}

export function syncPooledWindows(displays: Display[]): PooledOverlayWindow[] {
  const displayIds = new Set(displays.map(display => display.id));
  for (const pooled of pooledWindowsByDisplay.values()) {
    if (displayIds.has(pooled.displayId)) continue;
    removePooledWindow(pooled);
    if (!pooled.window.isDestroyed()) {
      pooled.window.destroy();
    }
  }

  return displays.map(getPooledWindow);
}

export function syncPooledWindowsForAllDisplays(): PooledOverlayWindow[] {
  return syncPooledWindows(screen.getAllDisplays());
}
