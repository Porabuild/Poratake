import { BrowserWindow, ipcMain, screen } from 'electron';
import type { Display, Rectangle } from 'electron';
import { pathToFileURL } from 'url';
import {
  captureRegionToFile,
  releaseRetainedDisplays,
} from '@/main/capture/screenshot/native-capture';
import {
  createOverlayWindow,
  previewPathFor,
  removePreview,
  sweepStalePreviews,
} from './window';
import type {
  AreaOverlayRect,
  AreaOverlayResult,
  AreaOverlayToolbar,
  AreaOverlayToolbarAction,
} from '@/types/area-overlay';

export interface OverlaySelection {
  display: Display;
  rect: Rectangle;
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
  previewPath: string | null;
  loaded: boolean;
  suppressed: boolean;
  imageUrl: string | null;
}

interface OverlaySession {
  entries: OverlayWindowEntry[];
  displays: Map<number, Display>;
  displayIdsByWebContents: Map<number, number>;
  focusDisplayId: number;
  freeze: boolean;
  interactive: boolean;
  showPrompt: boolean;
  aspectRatio: number | null;
  toolbar: AreaOverlayToolbar | null;
  hidden: boolean;
  selection: AreaOverlayResult | null;
  pendingPreset: AreaOverlayResult | null;
  callbacks: OverlayCallbacks;
  capturePromise: Promise<void> | null;
  resolve: (selection: OverlaySelection | null) => void;
}

let activeSession: OverlaySession | null = null;
let pendingTeardown: Promise<void> | null = null;
let ipcRegistered = false;

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

  ipcMain.on('area-overlay:ready', event => {
    const session = activeSession;
    const display = session && displayFor(session, event);
    if (!session || !display) return;

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

    finishSession(result, session);
  });

  ipcMain.on('area-overlay:cancel', event => {
    const session = activeSession;
    if (!session?.displayIdsByWebContents.has(event.sender.id)) return;
    cancelSession(session);
  });

  ipcMain.on('area-overlay:toolbar', (event, action: unknown) => {
    const session = activeSession;
    if (!session?.displayIdsByWebContents.has(event.sender.id)) return;
    if (!isValidToolbarAction(action)) return;

    session.callbacks.onToolbarAction?.(action);
  });
}

const SIMPLE_TOOLBAR_ACTIONS = new Set([
  'close',
  'screenshot',
  'record',
  'size-editor-opened',
  'size-editor-closed',
]);

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
  };

  switch (value.action) {
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
  if (entry.window.isDestroyed() || entry.window.isVisible()) return;

  entry.window.showInactive();

  if (displayId === session.focusDisplayId) {
    entry.window.focus();
  }
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
    if (!entry.window.isDestroyed()) {
      entry.window.hide();
    }
  }
}

function deliverOverlayParams(
  session: OverlaySession,
  entry: OverlayWindowEntry
): void {
  if (!entry.loaded || entry.window.isDestroyed()) return;
  if (session.freeze && !entry.imageUrl) return;

  const rect =
    session.selection?.displayId === entry.display.id
      ? session.selection
      : null;

  entry.window.webContents.send('load', {
    type: 'area-overlay',
    params: {
      displayId: entry.display.id,
      imageUrl: entry.imageUrl,
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

function discardEntry(entry: OverlayWindowEntry): void {
  if (!entry.window.isDestroyed()) {
    entry.window.destroy();
  }

  if (!entry.previewPath) return;

  removePreview(entry.previewPath);
}

async function releaseSession(session: OverlaySession): Promise<void> {
  if (!session.freeze) return;

  await session.capturePromise?.catch(() => undefined);
  await releaseRetainedDisplays();
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
): void {
  const session = activeSession;
  if (!session || session !== expectedSession) return;
  activeSession = null;

  session.entries.forEach(discardEntry);

  const display = result ? session.displays.get(result.displayId) : undefined;
  const release = session.freeze
    ? reserveTeardown(session)
    : () => Promise.resolve();

  if (!result || !display) {
    void release().catch(error => {
      console.error('Failed to release frozen displays:', error);
    });
    session.resolve(null);
    return;
  }

  session.resolve({
    ...toRegion(display, result),
    release,
  });
}

function cancelSession(session: OverlaySession): void {
  if (activeSession !== session) return;

  const { onCancelled } = session.callbacks;
  finishSession(null, session);
  onCancelled?.();
}

export function isOverlayActive(): boolean {
  return activeSession !== null;
}

export function cancelOverlaySelection(silent: boolean = false): void {
  const session = activeSession;
  if (!session) return;

  if (silent) {
    finishSession(null, session);
    return;
  }

  cancelSession(session);
}

export function confirmOverlaySelection(): void {
  const session = activeSession;
  if (!session) return;

  finishSession(session.selection, session);
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
      entry.window.showInactive();
      continue;
    }

    entry.window.hide();
  }
}

async function freezeEntry(
  session: OverlaySession,
  entry: OverlayWindowEntry
): Promise<void> {
  if (!entry.previewPath) return;

  const captured = await captureRegionToFile(
    entry.display.bounds,
    entry.previewPath,
    { retain: true }
  );

  if (activeSession !== session) {
    removePreview(entry.previewPath);
    return;
  }

  if (!captured) {
    discardEntry(entry);
    session.entries = session.entries.filter(item => item !== entry);
    return;
  }

  entry.imageUrl = pathToFileURL(entry.previewPath).href;
  deliverOverlayParams(session, entry);
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
  if (activeSession) {
    return null;
  }

  await pendingTeardown;

  if (activeSession) {
    return null;
  }

  const freeze = options?.freeze ?? true;

  registerOverlayIpc();

  if (freeze) {
    sweepStalePreviews();
  }

  const displays = screen.getAllDisplays();
  let resolveSession!: (selection: OverlaySelection | null) => void;
  const resultPromise = new Promise<OverlaySelection | null>(resolve => {
    resolveSession = resolve;
  });
  const session: OverlaySession = {
    entries: [],
    displays: new Map(displays.map(display => [display.id, display])),
    displayIdsByWebContents: new Map(),
    focusDisplayId: screen.getDisplayNearestPoint(screen.getCursorScreenPoint())
      .id,
    freeze,
    interactive: options?.interactive ?? false,
    showPrompt: options?.showPrompt ?? true,
    aspectRatio: options?.aspectRatio ?? null,
    toolbar: options?.toolbar ?? null,
    hidden: false,
    selection: presetSelection(displays, options?.preset),
    pendingPreset: null,
    callbacks: options?.callbacks ?? {},
    capturePromise: null,
    resolve: resolveSession,
  };
  session.pendingPreset = session.selection;
  activeSession = session;

  try {
    for (const display of displays) {
      const window = createOverlayWindow(display, freeze);
      const entry: OverlayWindowEntry = {
        window,
        display,
        previewPath: freeze ? previewPathFor(display) : null,
        loaded: false,
        suppressed: false,
        imageUrl: null,
      };
      session.entries.push(entry);
      session.displayIdsByWebContents.set(window.webContents.id, display.id);

      window.webContents.on('did-finish-load', () => {
        entry.loaded = true;
        deliverOverlayParams(session, entry);
      });

      window.webContents.on('render-process-gone', () => {
        cancelSession(session);
      });

      window.on('closed', () => {
        if (activeSession === session) {
          cancelSession(session);
        }
      });
    }

    if (session.selection) {
      hideInactiveOverlays(session, session.selection.displayId);
    }

    if (!freeze) {
      return resultPromise;
    }

    session.capturePromise = Promise.all(
      session.entries.map(entry => freezeEntry(session, entry))
    ).then(() => undefined);
    await session.capturePromise;

    if (activeSession === session && session.entries.length === 0) {
      finishSession(null, session);
    }

    return resultPromise;
  } catch (error) {
    finishSession(null, session);
    throw error;
  }
}
