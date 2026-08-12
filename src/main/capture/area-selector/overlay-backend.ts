import { screen } from 'electron';
import type { Rectangle } from 'electron';
import type { AreaSelection } from '@/types/area';
import type { AspectRatio } from '@/types/aspect-ratio';
import { listWindows } from '@/main/capture/window-selector';
import {
  cancelOverlaySelection,
  concealOverlayHandoff,
  confirmOverlaySelection,
  getOverlayWindowIds,
  hasOverlayHandoff,
  isOverlayActive,
  setOverlayAspectRatio,
  setOverlayPickTargets,
  setOverlayVisible,
  startInteractiveOverlay,
  updateOverlaySelection,
} from '@/main/capture/area-overlay';
import type {
  OverlayPickTarget,
  OverlayRegion,
} from '@/main/capture/area-overlay';
import { isWindows } from '@/main/utils/platform';
import type {
  AreaSelectionMode,
  ConfirmAreaSelectionOptions,
  PresetArea,
  StartAreaSelectionOptions,
} from './types';

let pendingAreaSelection: AreaSelection | null = null;
let callbacks: StartAreaSelectionOptions | undefined;
let pickedWindowNames: Map<number, string> | null = null;

function toAreaSelection(
  region: OverlayRegion,
  status: AreaSelection['status']
): AreaSelection {
  const windowId =
    region.pickId !== undefined && pickedWindowNames?.has(region.pickId)
      ? region.pickId
      : undefined;

  return {
    status,
    x: region.rect.x,
    y: region.rect.y,
    width: region.rect.width,
    height: region.rect.height,
    windowId,
    windowName:
      windowId === undefined ? undefined : pickedWindowNames?.get(windowId),
  };
}

export function updateAreaSelectionCallbacks(
  options: StartAreaSelectionOptions
): void {
  callbacks = options;
}

interface ResolvedStart {
  preset?: Rectangle;
  pickTargets?: OverlayPickTarget[];
  windowNames?: Map<number, string>;
  prompt?: string;
}

async function resolveWindowTargets(): Promise<ResolvedStart | null> {
  const overlayWindowIds = getOverlayWindowIds();
  const windows = (await listWindows()).filter(
    window => !overlayWindowIds.has(window.windowId)
  );

  if (windows.length === 0) {
    console.error('Window selection failed: no visible windows found');
    return null;
  }

  return {
    pickTargets: windows.map(window => ({
      id: window.windowId,
      rect: isWindows
        ? screen.screenToDipRect(null, window.bounds)
        : window.bounds,
    })),
    windowNames: new Map(
      windows.map(window => [window.windowId, window.title || window.ownerName])
    ),
    prompt: 'Click a window to select it · Esc to cancel',
  };
}

function resolveDisplayTargets(): ResolvedStart {
  return {
    pickTargets: screen.getAllDisplays().map(display => ({
      id: display.id,
      rect: display.bounds,
    })),
    prompt: 'Click a display to select it · Esc to cancel',
  };
}

async function resolveStart(
  options?: StartAreaSelectionOptions
): Promise<ResolvedStart | null> {
  const mode = options?.mode ?? 'manual';

  if (mode === 'manual') {
    return { preset: options?.preset };
  }

  if (mode === 'window') {
    return resolveWindowTargets();
  }

  if (screen.getAllDisplays().length === 1) {
    return { preset: screen.getPrimaryDisplay().bounds };
  }

  return resolveDisplayTargets();
}

async function resolveModeSwitch(
  mode: AreaSelectionMode
): Promise<ResolvedStart | null> {
  switch (mode) {
    case 'window':
      return resolveWindowTargets();
    case 'display':
      return resolveDisplayTargets();
    case 'manual':
      return {};
  }
}

export async function startAreaSelection(
  options?: StartAreaSelectionOptions
): Promise<AreaSelection | null> {
  const resolved = await resolveStart(options);

  if (resolved === null || isOverlayActive()) {
    return null;
  }

  callbacks = options;
  pendingAreaSelection = null;
  pickedWindowNames = resolved.windowNames ?? null;

  const selection = await startInteractiveOverlay({
    freeze: options?.freeze,
    visible: resolved.pickTargets ? true : options?.visible,
    preset: resolved.preset,
    pickTargets: resolved.pickTargets,
    prompt: resolved.prompt,
    showPrompt: options?.showPrompt ?? true,
    toolbar: options?.toolbar ?? null,
    callbacks: {
      onSelected: region => {
        pendingAreaSelection = toAreaSelection(region, 'selected');
        callbacks?.onSelected?.(pendingAreaSelection);
        callbacks?.onUpdate?.(pendingAreaSelection);
      },
      onUpdated: region => {
        pendingAreaSelection = toAreaSelection(region, 'updated');
        callbacks?.onUpdate?.(pendingAreaSelection);
      },
      onCancelled: () => {
        pendingAreaSelection = null;
        callbacks?.onCancelled?.();
        callbacks = undefined;
      },
      onToolbarAction: action => {
        callbacks?.onToolbarAction?.(action);
      },
    },
  });

  callbacks = undefined;

  if (!selection) {
    return null;
  }

  try {
    return toAreaSelection(selection, 'confirmed');
  } finally {
    await selection.release();
  }
}

export async function confirmAreaSelection(
  options?: ConfirmAreaSelectionOptions
): Promise<AreaSelection | null> {
  if (!pendingAreaSelection) {
    return null;
  }

  const selection: AreaSelection = {
    ...pendingAreaSelection,
    status: 'confirmed',
  };
  pendingAreaSelection = null;
  confirmOverlaySelection(options?.keepOverlayVisible ?? false);

  return selection;
}

export function concealAreaSelectorOverlay(): void {
  concealOverlayHandoff();
}

export function hasVisibleSelectorOverlay(): boolean {
  return hasOverlayHandoff();
}

export async function cancelAreaSelection(
  silent: boolean = false
): Promise<void> {
  pendingAreaSelection = null;
  await cancelOverlaySelection(silent);
}

export function hasPendingSelection(): boolean {
  return pendingAreaSelection !== null;
}

export async function hideAreaSelector(): Promise<void> {
  setOverlayVisible(false);
}

export async function showAreaSelector(): Promise<void> {
  setOverlayVisible(true);
}

export async function updateAreaSelection(
  bounds: PresetArea
): Promise<boolean> {
  if (!isOverlayActive()) {
    return false;
  }

  return updateOverlaySelection(bounds);
}

export async function setAreaSelectionMode(
  mode: AreaSelectionMode
): Promise<void> {
  if (!isOverlayActive()) {
    return;
  }

  const resolved = await resolveModeSwitch(mode);
  if (!resolved || !isOverlayActive()) {
    return;
  }

  pendingAreaSelection = null;
  pickedWindowNames = resolved.windowNames ?? null;
  setOverlayPickTargets(resolved.pickTargets ?? null, resolved.prompt ?? null);
}

export async function setAreaSelectorAspectRatio(
  ratio: AspectRatio
): Promise<void> {
  const value =
    ratio.width > 0 && ratio.height > 0 ? ratio.width / ratio.height : null;

  setOverlayAspectRatio(value);
}
