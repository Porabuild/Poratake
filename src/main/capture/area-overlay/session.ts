import { globalShortcut, ipcMain, screen } from 'electron';
import type { Display, Point, Rectangle, BrowserWindow } from 'electron';
import { freezeScreen, releaseScreen } from '@/main/capture/freeze-screen';
import { daemon } from '@/main/daemon';
import { flushPendingContinuations } from '@/main/utils/event-loop';
import { isWindows } from '@/main/utils/platform';
import { isDev } from '@/main/utils/env';
import { formatClock } from '@/main/utils/clock';
import {
  concealOverlayWindow,
  nativeWindowHandle,
  nativeWindowId,
  nextOverlayVisibilityVersion,
  pooledWindowForWebContents,
  removePooledWindow,
  showOverlayWindow,
  syncPooledWindows,
  syncPooledWindowsForAllDisplays,
  setOverlayWindowLostHandler,
  type PooledOverlayWindow,
} from './window-pool';
import { clipToDisplay, fitToDisplay, presetSelection } from './geometry';
import { getOverlayWindowIds } from './window-pool';
import { captureColorFrame, setColorPickerActive } from './color-picker';
import type { WindowLoadPayload } from '@/types/window-load';
import type {
  AreaOverlayPickTarget,
  AreaOverlayRenderer,
  AreaOverlayResult,
  AreaOverlayToolbar,
  AreaOverlayToolbarAction,
} from '@/types/area-overlay';

export interface OverlaySelection extends OverlayRegion {
  frozen: boolean;
  release: () => Promise<void>;
}

export interface OverlayRegion {
  display: Display;
  rect: Rectangle;
  pickId?: number;
}

export interface OverlayPickTarget {
  id: number;
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
  autoConfirm?: boolean;
  repeatablePicks?: boolean;
  renderer?: AreaOverlayRenderer;
  visible?: boolean;
  preset?: Rectangle;
  pickTargets?: OverlayPickTarget[];
  prompt?: string;
  showPrompt?: boolean;
  aspectRatio?: number | null;
  toolbar?: AreaOverlayToolbar | null;
  callbacks?: OverlayCallbacks;
}

interface OverlayWindowEntry {
  window: BrowserWindow;
  display: Display;
  loaded: boolean;
}

interface OverlaySession {
  entries: OverlayWindowEntry[];
  id: number;
  displays: Map<number, Display>;
  displayIdsByWebContents: Map<number, number>;
  freeze: boolean;
  requestedFreeze: boolean;
  freezeTransition: Promise<void>;
  interactive: boolean;
  autoConfirm: boolean;
  repeatablePicks: boolean;
  renderer: AreaOverlayRenderer;
  showPrompt: boolean;
  aspectRatio: number | null;
  toolbar: AreaOverlayToolbar | null;
  pickTargets: OverlayPickTarget[] | null;
  prompt: string | null;
  hidden: boolean;
  requestedVisible: boolean;
  visibilitySuspensions: number;
  selection: AreaOverlayResult | null;
  pendingPreset: AreaOverlayResult | null;
  freezeSettled: boolean;
  pendingReady: Set<number>;
  timingStart: number;
  revealCompletedAt: number | null;
  visibleAnnounced: boolean;
  overlayFocused: boolean;
  previousForegroundHandle: string | null;
  colorPickerActive: boolean;
  colorFramePaths: Map<number, string>;
  colorFrameVersions: Map<number, number>;
  callbacks: OverlayCallbacks;
  resolve: (selection: OverlaySelection | null) => void;
}

let activeSession: OverlaySession | null = null;
let pendingTeardown: Promise<void> | null = null;
let pendingRestore: Promise<void> | null = null;
let pendingHandoff: OverlayWindowEntry[] | null = null;
let startingSession = false;
let ipcRegistered = false;
let escapeShortcutRegistered = false;
let nextSessionId = 1;
const OVERLAY_REGION_INSET = 16;

function sessionEntryFor(
  session: OverlaySession,
  pooled: PooledOverlayWindow
): OverlayWindowEntry | undefined {
  return session.entries.find(entry => entry.window === pooled.window);
}

function mountWarmOverlay(pooled: PooledOverlayWindow): void {
  if (pooled.window.isDestroyed()) return;
  const payload: WindowLoadPayload = {
    type: 'area-overlay',
    params: {
      sessionId: 0,
      displayId: pooled.displayId,
      imageUrl: null,
      interactive: true,
      autoConfirm: true,
      repeatablePicks: false,
      showPrompt: true,
      aspectRatio: null,
      toolbar: null,
      rect: null,
      pickTargets: null,
      prompt: null,
    },
  };
  pooled.window.webContents.send('load', payload);
}

function setOverlayWindowRegion(
  session: OverlaySession,
  entry: OverlayWindowEntry
): Promise<void> {
  if (entry.window.isDestroyed() || !isWindows) return Promise.resolve();

  const windowHandle = nativeWindowHandle(entry.window);
  const selection =
    !session.freeze &&
    !session.hidden &&
    session.selection?.displayId === entry.display.id
      ? session.selection
      : null;
  const params = selection
    ? {
        windowHandle,
        x: selection.x,
        y: selection.y,
        width: selection.width,
        height: selection.height,
        windowWidth: entry.display.bounds.width,
        windowHeight: entry.display.bounds.height,
        inset: OVERLAY_REGION_INSET,
      }
    : { windowHandle };

  return daemon
    .call('area-selector', 'setWindowRegion', params)
    .then(() => undefined)
    .catch(error => {
      console.error('Failed to set overlay window region:', error);
    });
}

function syncOverlayWindowRegions(session: OverlaySession): void {
  for (const entry of session.entries) {
    void setOverlayWindowRegion(session, entry);
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

export function prewarmAreaOverlay(): void {
  registerOverlayIpc();
  syncPooledWindowsForAllDisplays();
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

  setOverlayWindowLostHandler(pooled => {
    const session = activeSession;
    if (session && sessionEntryFor(session, pooled)) {
      void cancelSession(session);
    }
  });

  ipcMain.on('area-overlay:renderer-mounted', event => {
    const pooled = pooledWindowForWebContents(event.sender.id);
    if (!pooled || pooled.window.isDestroyed()) return;
    pooled.window.webContents.send('area-overlay:prepare-renderer');
  });

  ipcMain.on('area-overlay:renderer-prepared', event => {
    const pooled = pooledWindowForWebContents(event.sender.id);
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
    const pooled = pooledWindowForWebContents(event.sender.id);
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

    if (!session.freezeSettled) {
      session.pendingReady.add(display.id);
      return;
    }

    void revealOverlay(session, display.id);
    announcePreset(session, display);
  });

  ipcMain.on('area-overlay:visible', (event, sessionId: unknown) => {
    const session = activeSession;
    if (!session || !session.displayIdsByWebContents.has(event.sender.id)) {
      return;
    }
    if (sessionId !== session.id) return;
    if (session.visibleAnnounced) return;

    session.visibleAnnounced = true;
    if (!isDev) return;

    const now = performance.now();
    const revealedAt = session.revealCompletedAt ?? session.timingStart;
    const paint = Math.round(now - revealedAt);
    const total = Math.round(now - session.timingStart);
    console.log(
      `[overlay-timing ${formatClock()}] paint=+${paint}ms total=${total}ms`
    );
  });

  ipcMain.on('area-overlay:selected', (event, result: unknown) => {
    const session = activeSession;
    const display = session && displayFor(session, event);
    if (!session || !display || !isValidResult(result, display)) return;

    adoptSelection(session, result, display);
    hideInactiveOverlays(session, display.id);
    session.callbacks.onSelected?.(toRegion(session, display, result));
  });

  ipcMain.on('area-overlay:updated', (event, result: unknown) => {
    const session = activeSession;
    const display = session && displayFor(session, event);
    if (!session || !display || !isValidResult(result, display)) return;

    adoptSelection(session, result, display);
    session.callbacks.onUpdated?.(toRegion(session, display, result));
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
    flushPendingContinuations();
  });

  ipcMain.on('area-overlay:color-picker', (event, active: unknown) => {
    const session = activeSession;
    if (!session?.displayIdsByWebContents.has(event.sender.id)) return;
    if (session.toolbar?.kind !== 'all-in-one') return;
    if (typeof active !== 'boolean') return;

    setColorPickerActive(session, active, enabled =>
      setEscapeShortcutEnabled(session, enabled)
    );
  });

  ipcMain.handle('area-overlay:color-picker-frame', async event => {
    const session = activeSession;
    const display = session && displayFor(session, event);
    if (
      !session ||
      !display ||
      session.toolbar?.kind !== 'all-in-one' ||
      !session.colorPickerActive
    ) {
      return null;
    }

    return captureColorFrame(session, display, () => activeSession === session);
  });
}

const SIMPLE_TOOLBAR_ACTIONS = new Set(['close']);

const HEX_COLOR_PATTERN = /^#[0-9a-f]{6}$/i;

function isValidToolbarAction(
  action: unknown
): action is AreaOverlayToolbarAction {
  if (!action || typeof action !== 'object') return false;

  const value = action as Partial<AreaOverlayToolbarAction> & {
    mode?: unknown;
    target?: unknown;
  };

  switch (value.action) {
    case 'select-capture-mode':
      return ['screenshot', 'record', 'ocr'].includes(value.mode as string);
    case 'select-capture-target':
      return ['area', 'window', 'screen'].includes(value.target as string);
    case 'copy-color':
      return (
        typeof (value as { color?: unknown }).color === 'string' &&
        HEX_COLOR_PATTERN.test((value as { color: string }).color)
      );
    default:
      return SIMPLE_TOOLBAR_ACTIONS.has(value.action as string);
  }
}

function revealOverlay(
  session: OverlaySession,
  displayId: number
): Promise<void> {
  if (session.hidden) return Promise.resolve();

  const entry = session.entries.find(item => item.display.id === displayId);
  if (!entry || entry.window.isDestroyed()) return Promise.resolve();

  return setOverlayWindowRegion(session, entry).then(() => {
    if (
      activeSession !== session ||
      session.hidden ||
      entry.window.isDestroyed()
    ) {
      return;
    }
    return showOverlayWindow(entry.window).then(() => {
      if (
        activeSession !== session ||
        session.hidden ||
        entry.window.isDestroyed()
      ) {
        return;
      }
      entry.window.moveTop();
      focusRevealedOverlay(session, entry);
      entry.window.webContents.send('area-overlay:revealed', session.id);
    });
  });
}

function focusRevealedOverlay(
  session: OverlaySession,
  entry: OverlayWindowEntry
): void {
  if (!isWindows || session.overlayFocused) {
    return;
  }

  const cursorDisplay = screen.getDisplayNearestPoint(
    screen.getCursorScreenPoint()
  );
  if (cursorDisplay.id !== entry.display.id) {
    return;
  }

  session.overlayFocused = true;

  void daemon
    .call<{ windowHandle: number | null }>(
      'area-selector',
      'getForegroundWindow'
    )
    .then(result => {
      if (activeSession !== session) return;
      const handle = result?.windowHandle ?? null;
      session.previousForegroundHandle =
        handle !== null && handle !== Number(nativeWindowId(entry.window))
          ? String(handle)
          : null;
      if (!entry.window.isDestroyed()) {
        entry.window.focus();
      }
    })
    .catch(() => {});
}

function restorePreviousFocus(session: OverlaySession): Promise<void> {
  if (!isWindows || !session.previousForegroundHandle) {
    return Promise.resolve();
  }

  return daemon
    .call('area-selector', 'setForegroundWindow', {
      windowHandle: session.previousForegroundHandle,
    })
    .then(() => undefined)
    .catch(() => {});
}

function announcePreset(session: OverlaySession, display: Display): void {
  const preset = session.pendingPreset;
  if (!preset || preset.displayId !== display.id) return;

  session.pendingPreset = null;
  session.callbacks.onSelected?.(toRegion(session, display, preset));
}

function toRegion(
  session: OverlaySession,
  display: Display,
  result: AreaOverlayResult
): OverlayRegion {
  const pickedRect =
    result.pickId === undefined
      ? undefined
      : session.pickTargets?.find(target => target.id === result.pickId)?.rect;

  return {
    display,
    rect: pickedRect ?? {
      x: display.bounds.x + result.x,
      y: display.bounds.y + result.y,
      width: result.width,
      height: result.height,
    },
    pickId: result.pickId,
  };
}

function adoptSelection(
  session: OverlaySession,
  result: AreaOverlayResult,
  display: Display
): void {
  session.pendingPreset = null;
  session.selection = result;
  broadcastRect(session, display.id);
  syncOverlayWindowRegions(session);
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
    if (entry.window.isDestroyed()) continue;

    entry.window.webContents.send('area-overlay:set-rect', { rect: null });
  }
}

function localPickTargets(
  session: OverlaySession,
  display: Display
): AreaOverlayPickTarget[] | null {
  if (!session.pickTargets) return null;

  const local: AreaOverlayPickTarget[] = [];
  for (const target of session.pickTargets) {
    const clipped = clipToDisplay(display, target.rect);
    if (clipped) local.push({ ...clipped, id: target.id });
  }
  return local;
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

  const payload: WindowLoadPayload = {
    type: session.renderer,
    params: {
      sessionId: session.id,
      displayId: entry.display.id,
      imageUrl: null,
      interactive: session.interactive,
      autoConfirm: session.autoConfirm,
      repeatablePicks: session.repeatablePicks,
      showPrompt: session.showPrompt,
      aspectRatio: session.aspectRatio,
      toolbar: session.toolbar,
      rect,
      pickTargets: localPickTargets(session, entry.display),
      prompt: session.prompt,
    },
  };
  entry.window.webContents.send('load', payload);
}

function isValidResult(
  result: unknown,
  display: Display
): result is AreaOverlayResult {
  if (!result || typeof result !== 'object') return false;

  const value = result as Partial<AreaOverlayResult>;
  if (value.displayId !== display.id) return false;
  if (value.pickId !== undefined && !Number.isFinite(value.pickId))
    return false;

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

function parkEntry(entry: OverlayWindowEntry): Promise<void> {
  if (entry.window.isDestroyed()) return Promise.resolve();
  entry.window.webContents.send('area-overlay:set-rect', { rect: null });
  return concealOverlayWindow(entry.window);
}

function handoffEntry(entry: OverlayWindowEntry): void {
  if (entry.window.isDestroyed()) return;
  nextOverlayVisibilityVersion(entry.window);
  entry.window.setIgnoreMouseEvents(true);
  entry.window.webContents.send('area-overlay:handoff');
}

export function hasOverlayHandoff(): boolean {
  return pendingHandoff !== null;
}

export function retainOverlayHandoffWindow(
  displayId: number
): BrowserWindow | null {
  const entries = pendingHandoff;
  const retained = entries?.find(entry => entry.display.id === displayId);
  if (!entries || !retained || retained.window.isDestroyed()) return null;

  entries.forEach(entry => {
    if (entry !== retained) parkEntry(entry);
  });
  pendingHandoff = [retained];
  return retained.window;
}

export function concealOverlayHandoff(): void {
  const entries = pendingHandoff;
  pendingHandoff = null;
  entries?.forEach(parkEntry);
}

async function releaseSession(session: OverlaySession): Promise<void> {
  await session.freezeTransition;
  if (!session.freeze) return;

  const released = await releaseScreen();
  if (released) {
    session.freeze = false;
  }
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
  expectedSession: OverlaySession | null = activeSession,
  keepVisible = false
): Promise<void> {
  const session = activeSession;
  if (!session || session !== expectedSession) return Promise.resolve();
  setColorPickerActive(session, false, enabled =>
    setEscapeShortcutEnabled(session, enabled)
  );
  activeSession = null;
  setEscapeShortcutEnabled(session, false);

  const parks: Promise<void>[] = [];
  if (keepVisible && result) {
    pendingHandoff = session.entries;
    session.entries.forEach(handoffEntry);
  } else {
    parks.push(...session.entries.map(entry => parkEntry(entry)));
  }

  const display = result ? session.displays.get(result.displayId) : undefined;
  const release = session.freeze
    ? reserveTeardown(session)
    : () => Promise.resolve();

  const foregroundCheck =
    isWindows && session.previousForegroundHandle
      ? daemon
          .call<{ windowHandle: number | null }>(
            'area-selector',
            'getForegroundWindow'
          )
          .catch(() => null)
      : Promise.resolve(null);

  pendingRestore = Promise.all(parks)
    .then(() => release())
    .then(async () => {
      const foreground = await foregroundCheck;
      if (
        foreground?.windowHandle === null ||
        foreground?.windowHandle === undefined
      ) {
        return;
      }
      if (!getOverlayWindowIds().has(foreground.windowHandle)) {
        return;
      }
      await restorePreviousFocus(session);
    })
    .catch(error => {
      console.error('Failed to release frozen displays:', error);
    });

  if (!result || !display) {
    const teardown = release().catch(error => {
      console.error('Failed to release frozen displays:', error);
    });
    session.resolve(null);
    return teardown;
  }

  session.resolve({
    ...toRegion(session, display, result),
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

export function getActiveOverlayWindowAtPoint(
  point: Point
): BrowserWindow | null {
  const session = activeSession;
  if (!session || session.hidden) return null;

  const display = screen.getDisplayNearestPoint(point);
  const entry = session.entries.find(item => item.display.id === display.id);
  if (!entry || entry.window.isDestroyed()) return null;

  return entry.window;
}

export function cancelOverlaySelection(silent: boolean = false): Promise<void> {
  const session = activeSession;
  if (!session) return Promise.resolve();

  if (silent) {
    return finishSession(null, session);
  }

  return cancelSession(session);
}

export function confirmOverlaySelection(keepVisible = false): void {
  const session = activeSession;
  if (!session) return;

  void finishSession(session.selection, session, keepVisible);
}

function raiseOverlayWindows(session: OverlaySession): void {
  if (session.hidden) return;

  for (const entry of session.entries) {
    if (entry.window.isDestroyed()) continue;
    entry.window.moveTop();
  }
}

export function setOverlayFreeze(enabled: boolean): Promise<void> {
  const session = activeSession;
  if (!session) return Promise.resolve();

  session.requestedFreeze = enabled;
  session.freezeTransition = session.freezeTransition
    .then(async () => {
      if (activeSession !== session) return;

      const requestedFreeze = session.requestedFreeze;
      if (requestedFreeze === session.freeze) return;

      if (requestedFreeze) {
        const frozen = await freezeScreen();
        if (activeSession !== session) {
          if (frozen) await releaseScreen();
          session.freeze = false;
          return;
        }
        session.freeze = frozen;
        if (frozen) {
          raiseOverlayWindows(session);
        }
      } else {
        const released = await releaseScreen();
        if (released) {
          session.freeze = false;
        }
      }

      syncOverlayWindowRegions(session);
    })
    .catch(error => {
      console.error('Failed to update overlay freeze state:', error);
    });

  return session.freezeTransition;
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
  syncOverlayWindowRegions(session);
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

export function setOverlayPickTargets(
  pickTargets: OverlayPickTarget[] | null,
  prompt: string | null,
  repeatablePicks: boolean = false
): void {
  const session = activeSession;
  if (!session) return;

  session.pickTargets = pickTargets;
  session.prompt = prompt;
  session.repeatablePicks = repeatablePicks;
  session.selection = null;
  session.pendingPreset = null;

  for (const entry of session.entries) {
    if (entry.window.isDestroyed()) continue;

    entry.window.webContents.send('area-overlay:set-pick-targets', {
      pickTargets: localPickTargets(session, entry.display),
      prompt,
      repeatablePicks,
    });
  }

  syncOverlayWindowRegions(session);
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

function applyOverlayVisibility(
  session: OverlaySession,
  visible: boolean
): void {
  if (activeSession !== session || session.hidden !== visible) return;
  session.hidden = !visible;

  for (const entry of session.entries) {
    if (entry.window.isDestroyed()) continue;

    if (visible) {
      void setOverlayWindowRegion(session, entry).then(() => {
        if (
          activeSession !== session ||
          session.hidden ||
          entry.window.isDestroyed()
        ) {
          return;
        }
        void showOverlayWindow(entry.window);
      });
      continue;
    }

    void concealOverlayWindow(entry.window);
  }
}

export function setOverlayVisible(visible: boolean): void {
  const session = activeSession;
  if (!session) return;

  session.requestedVisible = visible;
  applyOverlayVisibility(
    session,
    visible && session.visibilitySuspensions === 0
  );
}

export function suspendOverlayVisibility(): () => void {
  const session = activeSession;
  if (!session) return () => {};

  session.visibilitySuspensions += 1;
  applyOverlayVisibility(session, false);

  let restored = false;
  return () => {
    if (restored) return;
    restored = true;
    if (activeSession !== session) return;

    session.visibilitySuspensions -= 1;
    applyOverlayVisibility(
      session,
      session.requestedVisible && session.visibilitySuspensions === 0
    );
  };
}

export async function startOverlaySession(
  options?: OverlayOptions
): Promise<OverlaySelection | null> {
  if (activeSession || startingSession) {
    return null;
  }

  startingSession = true;

  try {
    const timingStart = performance.now();
    let timingTeardown = timingStart;
    let timingInit = timingStart;
    let timingConceal = timingStart;
    let timingFreeze = timingStart;

    await pendingTeardown;
    await pendingRestore;
    timingTeardown = performance.now();

    if (activeSession) {
      return null;
    }

    pendingHandoff = null;
    registerOverlayIpc();
    const displays = screen.getAllDisplays();
    const pooledWindows = syncPooledWindows(displays);
    const requestedFreeze = options?.freeze ?? true;

    await Promise.all(pooledWindows.map(pooled => pooled.initialized));
    timingInit = performance.now();
    await Promise.all(
      pooledWindows.map(pooled =>
        pooled.window.isVisible()
          ? concealOverlayWindow(pooled.window)
          : Promise.resolve()
      )
    );
    timingConceal = performance.now();

    let resolveSession!: (selection: OverlaySelection | null) => void;
    const resultPromise = new Promise<OverlaySelection | null>(resolve => {
      resolveSession = resolve;
    });
    const session: OverlaySession = {
      entries: [],
      id: nextSessionId++,
      displays: new Map(displays.map(display => [display.id, display])),
      displayIdsByWebContents: new Map(),
      freeze: false,
      requestedFreeze,
      freezeTransition: Promise.resolve(),
      interactive: options?.interactive ?? false,
      autoConfirm: options?.autoConfirm ?? true,
      repeatablePicks: options?.repeatablePicks ?? false,
      renderer: options?.renderer ?? 'area-overlay',
      showPrompt: options?.showPrompt ?? true,
      aspectRatio: options?.aspectRatio ?? null,
      toolbar: options?.toolbar ?? null,
      pickTargets: options?.pickTargets ?? null,
      prompt: options?.prompt ?? null,
      hidden: options?.visible === false,
      requestedVisible: options?.visible !== false,
      visibilitySuspensions: 0,
      selection: presetSelection(displays, options?.preset),
      pendingPreset: null,
      freezeSettled: !requestedFreeze,
      pendingReady: new Set(),
      timingStart,
      revealCompletedAt: null,
      visibleAnnounced: false,
      overlayFocused: false,
      previousForegroundHandle: null,
      colorPickerActive: false,
      colorFramePaths: new Map(),
      colorFrameVersions: new Map(),
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
        return resultPromise;
      }

      if (requestedFreeze) {
        const freeze = await freezeScreen();
        if (activeSession !== session) {
          if (freeze) await releaseScreen();
          return resultPromise;
        }
        session.freeze = freeze;
      }
      timingFreeze = performance.now();

      session.freezeSettled = true;
      const reveals: Promise<void>[] = [];
      for (const displayId of session.pendingReady) {
        const display = session.displays.get(displayId);
        if (!display) continue;
        reveals.push(revealOverlay(session, displayId));
        announcePreset(session, display);
      }
      session.pendingReady.clear();

      void Promise.all(reveals).then(() => {
        session.revealCompletedAt = performance.now();
        if (!isDev || reveals.length === 0) {
          return;
        }
        const elapsed = (mark: number) => Math.round(mark - timingStart);
        const total = Math.round(session.revealCompletedAt - timingStart);
        console.log(
          `[overlay-timing ${formatClock()}] teardown=${elapsed(
            timingTeardown
          )} init=${elapsed(timingInit)} conceal=${elapsed(
            timingConceal
          )} freeze=${elapsed(timingFreeze)} reveal=${
            total - elapsed(timingFreeze)
          } total=${total}ms`
        );
      });

      return resultPromise;
    } catch (error) {
      await finishSession(null, session);
      throw error;
    }
  } finally {
    startingSession = false;
  }
}
