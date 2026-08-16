import { screen } from 'electron';
import type { Rectangle } from 'electron';
import type { AreaSelection } from '@/types/area';
import type { AspectRatio } from '@/types/aspect-ratio';
import {
  cancelOverlaySelection,
  concealOverlayHandoff,
  confirmOverlaySelection,
  hasOverlayHandoff,
  isOverlayActive,
  setOverlayFreeze,
  resolveWindowPickTargets,
  setOverlayAspectRatio,
  setOverlayPickTargets,
  setOverlayVisible,
  startInteractiveOverlay,
  updateOverlaySelection,
} from '@/main/capture/area-overlay';
import type {
  OverlayCallbacks,
  OverlayPickTarget,
  OverlayRegion,
  OverlaySelection,
} from '@/main/capture/area-overlay';
import type {
  AreaSelectionMode,
  ConfirmAreaSelectionOptions,
  PresetArea,
  StartAreaSelectionOptions,
} from './types';

let pendingAreaSelection: AreaSelection | null = null;
let callbacks: StartAreaSelectionOptions | undefined;
let pickedWindowNames: Map<number, string> | null = null;
let pickedWindowBounds: Map<number, Rectangle> | null = null;
let modeSwitchVersion = 0;

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
    screenId: region.display.id,
    windowId,
    windowName:
      windowId === undefined ? undefined : pickedWindowNames?.get(windowId),
    windowBounds:
      windowId === undefined ? undefined : pickedWindowBounds?.get(windowId),
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
  repeatablePicks?: boolean;
  windowNames?: Map<number, string>;
  windowBounds?: Map<number, Rectangle>;
  prompt?: string;
}

async function resolveWindowTargets(): Promise<ResolvedStart | null> {
  const resolved = await resolveWindowPickTargets();
  if (!resolved) {
    return null;
  }

  return {
    pickTargets: resolved.targets,
    windowNames: resolved.names,
    windowBounds: resolved.captureRects,
    prompt: resolved.prompt,
  };
}

function resolveDisplayTargets(): ResolvedStart {
  return {
    pickTargets: screen.getAllDisplays().map(display => ({
      id: display.id,
      rect: display.bounds,
    })),
    repeatablePicks: true,
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

  if (!options?.requireDisplayPick && screen.getAllDisplays().length === 1) {
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

function overlayCallbacks(): OverlayCallbacks {
  return {
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
  };
}

function beginInteractiveOverlay(
  options: StartAreaSelectionOptions | undefined,
  resolved: ResolvedStart
): Promise<OverlaySelection | null> {
  callbacks = options;
  pendingAreaSelection = null;
  pickedWindowNames = resolved.windowNames ?? null;
  pickedWindowBounds = resolved.windowBounds ?? null;

  return startInteractiveOverlay({
    freeze: options?.freeze,
    renderer: options?.renderer,
    autoConfirm: false,
    repeatablePicks: resolved.repeatablePicks ?? false,
    visible: resolved.pickTargets ? true : options?.visible,
    preset: resolved.preset,
    pickTargets: resolved.pickTargets,
    prompt: resolved.prompt,
    showPrompt: options?.showPrompt ?? true,
    toolbar: options?.toolbar ?? null,
    callbacks: overlayCallbacks(),
  });
}

async function finishSelection(
  selection: OverlaySelection | null
): Promise<AreaSelection | null> {
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

export async function startAreaSelection(
  options?: StartAreaSelectionOptions
): Promise<AreaSelection | null> {
  const requestVersion = ++modeSwitchVersion;

  if (isOverlayActive()) {
    return null;
  }

  const mode = options?.mode ?? 'manual';

  if (mode === 'window') {
    const prompt = 'Click a window to select it · Esc to cancel';
    const selectionPromise = beginInteractiveOverlay(options, {
      pickTargets: [],
      prompt,
    });

    void resolveWindowTargets().then(resolved => {
      if (requestVersion !== modeSwitchVersion) {
        return;
      }

      if (!resolved) {
        void cancelAreaSelection(true);
        return;
      }

      pickedWindowNames = resolved.windowNames ?? null;
      pickedWindowBounds = resolved.windowBounds ?? null;
      setOverlayPickTargets(
        resolved.pickTargets ?? [],
        resolved.prompt ?? prompt,
        resolved.repeatablePicks ?? false
      );
    });

    return finishSelection(await selectionPromise);
  }

  const resolved = await resolveStart(options);

  if (
    resolved === null ||
    isOverlayActive() ||
    requestVersion !== modeSwitchVersion
  ) {
    return null;
  }

  return finishSelection(await beginInteractiveOverlay(options, resolved));
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
  modeSwitchVersion += 1;
  confirmOverlaySelection(options?.keepOverlayVisible ?? false);

  return selection;
}

export function concealAreaSelectorOverlay(): void {
  concealOverlayHandoff();
}

export function setAreaSelectorFreeze(enabled: boolean): Promise<void> {
  return setOverlayFreeze(enabled);
}

export function hasVisibleSelectorOverlay(): boolean {
  return hasOverlayHandoff();
}

export async function cancelAreaSelection(
  silent: boolean = false
): Promise<void> {
  modeSwitchVersion += 1;
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

  const requestVersion = ++modeSwitchVersion;
  const resolved = await resolveModeSwitch(mode);
  if (requestVersion !== modeSwitchVersion || !resolved || !isOverlayActive()) {
    return;
  }

  pendingAreaSelection = null;
  pickedWindowNames = resolved.windowNames ?? null;
  pickedWindowBounds = resolved.windowBounds ?? null;
  setOverlayPickTargets(
    resolved.pickTargets ?? null,
    resolved.prompt ?? null,
    resolved.repeatablePicks ?? false
  );
}

export async function setAreaSelectorAspectRatio(
  ratio: AspectRatio
): Promise<void> {
  const value =
    ratio.width > 0 && ratio.height > 0 ? ratio.width / ratio.height : null;

  setOverlayAspectRatio(value);
}
