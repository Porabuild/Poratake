import { BrowserWindow, globalShortcut, ipcMain, screen } from 'electron';
import type { Display, Rectangle } from 'electron';
import { freezeScreen, releaseScreen } from '@/main/capture/freeze-screen';
import { daemon } from '@/main/daemon';
import { createOverlayWindow } from './window';
import type {
  AreaOverlayRect,
  AreaOverlayResult,
  AreaOverlayToolbar,
  AreaOverlayToolbarAction,
} from '@/types/area-overlay';

export interface OverlaySelection {
  display: Display;
  rect: Rectangle;
  frozen: boolean;
  release: () => Promise<void>;
}

export interface OverlayRegion {
  display: Display;
  rect: Rectangle;
}

export interface OverlayCallbacks {
  onSelected?: (region: OverlayRegion) => void;
  onUpdated?: (region: OverlayRegion) => void;
  onCancelled?: () => void;
  onToolbarAction?: (action: AreaOverlayToolbarAction) => void;
}

export interface OverlayOptions {
  freeze?: boolean;
  interactive?: boolean;
  preset?: Rectangle;
  showPrompt?: boolean;
  aspectRatio?: number | null;
  toolbar?: AreaOverlayToolbar | null;
  callbacks?: OverlayCallbacks;
}

interface OverlayWindowEntry {
  window: BrowserWindow;
  display: Display;
  loaded: boolean;
  suppressed: boolean;
}

interface PooledOverlayWindow {
  window: BrowserWindow;
  displayId: number;
  prepared: boolean;
  transitionsReady: Promise<void>;
}

interface OverlaySession {
  entries: OverlayWindowEntry[];
  id: number;
  displays: Map<number, Display>;
  displayIdsByWebContents: Map<number, number>;
  freeze: boolean;
  interactive: boolean;
  showPrompt: boolean;
  aspectRatio: number | null;
  toolbar: AreaOverlayToolbar | null;
  hidden: boolean;
  selection: AreaOverlayResult | null;
  pendingPreset: AreaOverlayResult | null;
  callbacks: OverlayCallbacks;
  resolve: (selection: OverlaySelection | null) => void;
}

let activeSession: OverlaySession | null = null;
let pendingTeardown: Promise<void> | null = null;
let startingSession = false;
let ipcRegistered = false;
let escapeShortcutRegistered = false;
let nextSessionId = 1;
const pooledWindowsByDisplay = new Map<number, PooledOverlayWindow>();
const pooledWindowsByWebContents = new Map<number, PooledOverlayWindow>();

function removePooledWindow(entry: PooledOverlayWindow): void {
  if (pooledWindowsByDisplay.get(entry.displayId) === entry) {
    pooledWindowsByDisplay.delete(entry.displayId);
  }
  pooledWindowsByWebContents.delete(entry.window.webContents.id);
}

function sessionEntryFor(
  session: OverlaySession,
  pooled: PooledOverlayWindow
): OverlayWindowEntry | undefined {
  return session.entries.find(entry => entry.window === pooled.window);
}

function mountWarmOverlay(pooled: PooledOverlayWindow): void {
  if (pooled.window.isDestroyed()) return;
  pooled.window.webContents.send('load', {
    type: 'area-overlay',
    params: {
      sessionId: 0,
      displayId: pooled.displayId,
      imageUrl: null,
      interactive: true,
      showPrompt: true,
      aspectRatio: null,
      toolbar: null,
      rect: null,
    },
  });
}

function createPooledWindow(display: Display): PooledOverlayWindow {
  const window = createOverlayWindow(display);
  const windowHandle = window
    .getNativeWindowHandle()
    .readBigUInt64LE()
    .toString();
  const pooled: PooledOverlayWindow = {
    window,
    displayId: display.id,
    prepared: false,
    transitionsReady: daemon
      .call('area-selector', 'disableWindowTransitions', { windowHandle })
      .then(() => undefined)
      .catch(error => {
        console.error('Failed to disable overlay window transitions:', error);
      }),
  };
  pooledWindowsByDisplay.set(display.id, pooled);
  pooledWindowsByWebContents.set(window.webContents.id, pooled);

  window.webContents.on('render-process-gone', () => {
    removePooledWindow(pooled);
    const session = activeSession;
    if (session && sessionEntryFor(session, pooled)) {
      void cancelSession(session);
    }
  });

  window.on('closed', () => {
    removePooledWindow(pooled);
    const session = activeSession;
    if (session && sessionEntryFor(session, pooled)) {
      void cancelSession(session);
    }
  });

  return pooled;
}

function concealOverlayWindow(window: BrowserWindow): void {
  if (window.isDestroyed()) return;
  window.setOpacity(0);
  window.setIgnoreMouseEvents(true);
  window.hide();
}

function showOverlayWindow(window: BrowserWindow): void {
  window.setOpacity(1);
  window.setIgnoreMouseEvents(false);
  if (!window.isVisible()) {
    window.showInactive();
  }
}

function stageOverlayWindow(window: BrowserWindow): void {
  if (window.isDestroyed()) return;
  window.setOpacity(0);
  window.setIgnoreMouseEvents(false);
  if (!window.isVisible()) {
    window.showInactive();
  }
}

function setEscapeShortcutEnabled(
  session: OverlaySession,
  enabled: boolean
): void {
  if (!enabled) {
    if (!escapeShortcutRegistered) return;
    globalShortcut.unregister('Escape');
    escapeShortcutRegistered = false;
    return;
  }

  if (escapeShortcutRegistered) return;
  escapeShortcutRegistered = globalShortcut.register('Escape', () => {
    void cancelSession(session);
  });
}

function getPooledWindow(display: Display): PooledOverlayWindow {
  const existing = pooledWindowsByDisplay.get(display.id);
  const pooled =
    existing && !existing.window.isDestroyed()
      ? existing
      : createPooledWindow(display);

  pooled.window.setBounds(display.bounds);
  concealOverlayWindow(pooled.window);

  return pooled;
}

function syncPooledWindows(displays: Display[]): PooledOverlayWindow[] {
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

export function prewarmAreaOverlay(): void {
  registerOverlayIpc();
  syncPooledWindows(screen.getAllDisplays());
}

function displayFor(
  session: OverlaySession,
  event: { sender: { id: number } }
): Display | null {
  const displayId = session.displayIdsByWebContents.get(event.sender.id);
  if (displayId === undefined) return null;
  return session.displays.get(displayId) ?? null;
}

function registerOverlayIpc(): void {
  if (ipcRegistered) return;
  ipcRegistered = true;

  ipcMain.on('area-overlay:renderer-mounted', event => {
    const pooled = pooledWindowsByWebContents.get(event.sender.id);
    if (!pooled || pooled.window.isDestroyed()) return;
    pooled.window.webContents.send('area-overlay:prepare-renderer');
  });

  ipcMain.on('area-overlay:renderer-prepared', event => {
    const pooled = pooledWindowsByWebContents.get(event.sender.id);
    if (!pooled) return;
    pooled.prepared = true;

    const session = activeSession;
    const entry = session && sessionEntryFor(session, pooled);
    if (!session || !entry) {
      mountWarmOverlay(pooled);
      return;
    }

    entry.loaded = true;
    deliverOverlayParams(session, entry);
  });

  ipcMain.on('area-overlay:renderer-failed', event => {
    const pooled = pooledWindowsByWebContents.get(event.sender.id);
    if (!pooled) return;
    removePooledWindow(pooled);
    if (!pooled.window.isDestroyed()) {
      pooled.window.destroy();
    }

    const session = activeSession;
    if (session && sessionEntryFor(session, pooled)) {
      void cancelSession(session);
    }
  });

  ipcMain.on('area-overlay:ready', (event, sessionId: unknown) => {
    const session = activeSession;
    const display = session && displayFor(session, event);
    if (!session || !display) return;
    if (typeof sessionId === 'number' && sessionId !== session.id) return;

    revealOverlay(session, display.id);
    announcePreset(session, display);
  });

  ipcMain.on('area-overlay:selected', (event, result: unknown) => {
    const session = activeSession;
    const display = session && displayFor(session, event);
    if (!session || !display || !isValidResult(result, display)) return;

    adoptSelection(session, result, display);
    hideInactiveOverlays(session, display.id);
    session.callbacks.onSelected?.(toRegion(display, result));
  });

  ipcMain.on('area-overlay:updated', (event, result: unknown) => {
    const session = activeSession;
    const display = session && displayFor(session, event);
    if (!session || !display || !isValidResult(result, display)) return;

    adoptSelection(session, result, display);
    session.callbacks.onUpdated?.(toRegion(display, result));
  });

  ipcMain.on('area-overlay:confirm', (event, result: unknown) => {
    const session = activeSession;
    const display = session && displayFor(session, event);
    if (!session || !display || !isValidResult(result, display)) return;

    void finishSession(result, session);
  });

  ipcMain.on('area-overlay:cancel', event => {
    const session = activeSession;
    if (!session?.displayIdsByWebContents.has(event.sender.id)) return;
    void cancelSession(session);
  });

  ipcMain.on('area-overlay:toolbar', (event, action: unknown) => {
    const session = activeSession;
    if (!session?.displayIdsByWebContents.has(event.sender.id)) return;
    if (!isValidToolbarAction(action)) return;

    session.callbacks.onToolbarAction?.(action);
  });

  ipcMain.on('area-overlay:color-picker', (event, active: unknown) => {
    const session = activeSession;
    if (!session?.displayIdsByWebContents.has(event.sender.id)) return;
    if (typeof active !== 'boolean') return;

    setEscapeShortcutEnabled(session, !active);
  });
}

const SIMPLE_TOOLBAR_ACTIONS = new Set([
  'close',
  'screenshot',
  'record',
  'ocr',
  'size-editor-opened',
  'size-editor-closed',
]);

const HEX_COLOR_PATTERN = /^#[0-9a-f]{6}$/i;

function hasNumericSize(value: { width?: unknown; height?: unknown }): boolean {
  return (
    typeof value.width === 'number' &&
    Number.isFinite(value.width) &&
    typeof value.height === 'number' &&
    Number.isFinite(value.height)
  );
}

function isValidToolbarAction(
  action: unknown
): action is AreaOverlayToolbarAction {
  if (!action || typeof action !== 'object') return false;

  const value = action as Partial<AreaOverlayToolbarAction> & {
    width?: unknown;
    height?: unknown;
    name?: unknown;
    mode?: unknown;
  };

  switch (value.action) {
    case 'select-capture-mode':
      return ['screenshot', 'record', 'ocr'].includes(value.mode as string);
    case 'copy-color':
      return (
        typeof (value as { color?: unknown }).color === 'string' &&
        HEX_COLOR_PATTERN.test((value as { color: string }).color)
      );
    case 'update-size':
      return hasNumericSize(value);
    case 'select-aspect-ratio':
      return typeof value.name === 'string' && hasNumericSize(value);
    default:
      return SIMPLE_TOOLBAR_ACTIONS.has(value.action as string);
  }
}

function revealOverlay(session: OverlaySession, displayId: number): void {
  const entry = session.entries.find(item => item.display.id === displayId);
  if (!entry || entry.suppressed) return;
  if (entry.window.isDestroyed()) return;

  showOverlayWindow(entry.window);
  entry.window.moveTop();
}

function announcePreset(session: OverlaySession, display: Display): void {
  const preset = session.pendingPreset;
  if (!preset || preset.displayId !== display.id) return;

  session.pendingPreset = null;
  session.callbacks.onSelected?.(toRegion(display, preset));
}

function toRegion(display: Display, rect: AreaOverlayRect): OverlayRegion {
  return {
    display,
    rect: {
      x: display.bounds.x + rect.x,
      y: display.bounds.y + rect.y,
      width: rect.width,
      height: rect.height,
    },
  };
}

function adoptSelection(
  session: OverlaySession,
  result: AreaOverlayResult,
  display: Display
): void {
  session.selection = result;
  broadcastRect(session, display.id);
}

function broadcastRect(session: OverlaySession, skipDisplayId?: number): void {
  for (const entry of session.entries) {
    if (entry.display.id === skipDisplayId || entry.window.isDestroyed()) {
      continue;
    }

    const rect =
      session.selection?.displayId === entry.display.id
        ? session.selection
        : null;

    entry.window.webContents.send('area-overlay:set-rect', { rect });
  }
}

function hideInactiveOverlays(
  session: OverlaySession,
  activeDisplayId: number
): void {
  if (!session.interactive) return;

  for (const entry of session.entries) {
    if (entry.display.id === activeDisplayId) continue;

    entry.suppressed = true;
    concealOverlayWindow(entry.window);
  }
}

function deliverOverlayParams(
  session: OverlaySession,
  entry: OverlayWindowEntry
): void {
  if (!entry.loaded || entry.window.isDestroyed()) return;

  const rect =
    session.selection?.displayId === entry.display.id
      ? session.selection
      : null;

  entry.window.webContents.send('load', {
    type: 'area-overlay',
    params: {
      sessionId: session.id,
      displayId: entry.display.id,
      imageUrl: null,
      interactive: session.interactive,
      showPrompt: session.showPrompt,
      aspectRatio: session.aspectRatio,
      toolbar: session.toolbar,
      rect,
    },
  });
}

function isValidResult(
  result: unknown,
  display: Display
): result is AreaOverlayResult {
  if (!result || typeof result !== 'object') return false;

  const value = result as Partial<AreaOverlayResult>;
  if (value.displayId !== display.id) return false;

  const coordinates = [value.x, value.y, value.width, value.height];
  if (
    !coordinates.every(
      item => typeof item === 'number' && Number.isFinite(item)
    )
  ) {
    return false;
  }

  const { x, y, width, height } = value as AreaOverlayResult;
  if (x < 0 || y < 0 || width <= 0 || height <= 0) return false;

  return (
    x + width <= display.bounds.width && y + height <= display.bounds.height
  );
}

function parkEntry(entry: OverlayWindowEntry): void {
  if (entry.window.isDestroyed()) return;
  entry.window.webContents.send('area-overlay:set-rect', { rect: null });
  concealOverlayWindow(entry.window);
}

async function releaseSession(session: OverlaySession): Promise<void> {
  if (!session.freeze) return;

  await releaseScreen();
}

function reserveTeardown(session: OverlaySession): () => Promise<void> {
  let releasePromise: Promise<void> | null = null;
  let finishTeardown!: () => void;
  const teardown = new Promise<void>(resolve => {
    finishTeardown = resolve;
  });
  pendingTeardown = teardown;

  return () => {
    if (!releasePromise) {
      releasePromise = releaseSession(session).finally(() => {
        finishTeardown();
        if (pendingTeardown === teardown) {
          pendingTeardown = null;
        }
      });
    }
    return releasePromise;
  };
}

function finishSession(
  result: AreaOverlayResult | null,
  expectedSession: OverlaySession | null = activeSession
): Promise<void> {
  const session = activeSession;
  if (!session || session !== expectedSession) return Promise.resolve();
  activeSession = null;
  setEscapeShortcutEnabled(session, false);

  session.entries.forEach(parkEntry);

  const display = result ? session.displays.get(result.displayId) : undefined;
  const release = session.freeze
    ? reserveTeardown(session)
    : () => Promise.resolve();

  if (!result || !display) {
    const teardown = release().catch(error => {
      console.error('Failed to release frozen displays:', error);
    });
    session.resolve(null);
    return teardown;
  }

  session.resolve({
    ...toRegion(display, result),
    frozen: session.freeze,
    release,
  });
  return Promise.resolve();
}

function cancelSession(session: OverlaySession): Promise<void> {
  if (activeSession !== session) return Promise.resolve();

  const { onCancelled } = session.callbacks;
  const teardown = finishSession(null, session);
  onCancelled?.();
  return teardown;
}

export function isOverlayActive(): boolean {
  return activeSession !== null;
}

export function cancelOverlaySelection(silent: boolean = false): Promise<void> {
  const session = activeSession;
  if (!session) return Promise.resolve();

  if (silent) {
    return finishSession(null, session);
  }

  return cancelSession(session);
}

export function confirmOverlaySelection(): void {
  const session = activeSession;
  if (!session) return;

  void finishSession(session.selection, session);
}

export function updateOverlaySelection(rect: Rectangle): boolean {
  const session = activeSession;
  if (!session) return false;

  const display = screen.getDisplayMatching(rect);
  const entry = session.entries.find(item => item.display.id === display.id);
  if (!entry) return false;

  const local = fitToDisplay(display, rect);
  if (!local) return false;

  session.selection = local;
  broadcastRect(session);
  return true;
}

export function setOverlayAspectRatio(ratio: number | null): void {
  const session = activeSession;
  if (!session) return;

  session.aspectRatio = ratio;

  for (const entry of session.entries) {
    if (entry.window.isDestroyed()) continue;
    entry.window.webContents.send('area-overlay:set-aspect-ratio', {
      aspectRatio: ratio,
    });
  }
}

export function setOverlayToolbar(toolbar: AreaOverlayToolbar | null): void {
  const session = activeSession;
  if (!session) return;

  session.toolbar = toolbar;

  for (const entry of session.entries) {
    if (entry.window.isDestroyed()) continue;
    entry.window.webContents.send('area-overlay:set-toolbar', { toolbar });
  }
}

export function setOverlayVisible(visible: boolean): void {
  const session = activeSession;
  if (!session || session.hidden !== visible) return;

  session.hidden = !visible;

  for (const entry of session.entries) {
    if (entry.window.isDestroyed() || entry.suppressed) continue;

    if (visible) {
      showOverlayWindow(entry.window);
      continue;
    }

    concealOverlayWindow(entry.window);
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function fitToDisplay(
  display: Display,
  rect: Rectangle
): AreaOverlayResult | null {
  const { width: displayWidth, height: displayHeight } = display.bounds;
  const width = clamp(Math.round(rect.width), 1, displayWidth);
  const height = clamp(Math.round(rect.height), 1, displayHeight);

  if (!Number.isFinite(rect.x) || !Number.isFinite(rect.y)) return null;

  return {
    displayId: display.id,
    width,
    height,
    x: clamp(Math.round(rect.x - display.bounds.x), 0, displayWidth - width),
    y: clamp(Math.round(rect.y - display.bounds.y), 0, displayHeight - height),
  };
}

function presetSelection(
  displays: Display[],
  preset?: Rectangle
): AreaOverlayResult | null {
  if (!preset) return null;

  const display = screen.getDisplayMatching(preset);
  if (!displays.some(item => item.id === display.id)) return null;

  return fitToDisplay(display, preset);
}

export async function startOverlaySession(
  options?: OverlayOptions
): Promise<OverlaySelection | null> {
  if (activeSession || startingSession) {
    return null;
  }

  startingSession = true;

  try {
    await pendingTeardown;

    if (activeSession) {
      return null;
    }

    registerOverlayIpc();
    const displays = screen.getAllDisplays();
    const pooledWindows = syncPooledWindows(displays);
    await Promise.all(pooledWindows.map(window => window.transitionsReady));
    pooledWindows.forEach(window => stageOverlayWindow(window.window));

    const requestedFreeze = options?.freeze ?? true;
    const freeze = requestedFreeze ? await freezeScreen() : false;

    let resolveSession!: (selection: OverlaySelection | null) => void;
    const resultPromise = new Promise<OverlaySelection | null>(resolve => {
      resolveSession = resolve;
    });
    const session: OverlaySession = {
      entries: [],
      id: nextSessionId++,
      displays: new Map(displays.map(display => [display.id, display])),
      displayIdsByWebContents: new Map(),
      freeze,
      interactive: options?.interactive ?? false,
      showPrompt: options?.showPrompt ?? true,
      aspectRatio: options?.aspectRatio ?? null,
      toolbar: options?.toolbar ?? null,
      hidden: false,
      selection: presetSelection(displays, options?.preset),
      pendingPreset: null,
      callbacks: options?.callbacks ?? {},
      resolve: resolveSession,
    };
    session.pendingPreset = session.selection;
    activeSession = session;
    setEscapeShortcutEnabled(session, true);

    try {
      for (const [index, display] of displays.entries()) {
        const pooled = pooledWindows[index];
        if (pooled.window.isDestroyed()) continue;
        const window = pooled.window;
        const entry: OverlayWindowEntry = {
          window,
          display,
          loaded: pooled.prepared,
          suppressed: false,
        };
        session.entries.push(entry);
        session.displayIdsByWebContents.set(window.webContents.id, display.id);

        if (entry.loaded) {
          deliverOverlayParams(session, entry);
        }
      }

      if (session.selection) {
        hideInactiveOverlays(session, session.selection.displayId);
      }

      if (session.entries.length === 0) {
        void finishSession(null, session);
      }

      return resultPromise;
    } catch (error) {
      await finishSession(null, session);
      throw error;
    }
  } finally {
    startingSession = false;
  }
}
