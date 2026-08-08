import { screen } from 'electron';
import type { Rectangle } from 'electron';
import type { AreaSelection } from '@/types/area';
import type { AspectRatio } from '@/types/aspect-ratio';
import {
  displayFromSelection,
  selectDisplay,
} from '@/main/capture/display-selector';
import { selectWindow } from '@/main/capture/window-selector';
import {
  cancelOverlaySelection,
  confirmOverlaySelection,
  isOverlayActive,
  setOverlayAspectRatio,
  setOverlayVisible,
  startInteractiveOverlay,
  updateOverlaySelection,
} from '@/main/capture/area-overlay';
import type { OverlayRegion } from '@/main/capture/area-overlay';
import { isWindows } from '@/main/utils/platform';
import type { PresetArea, StartAreaSelectionOptions } from './types';

let pendingAreaSelection: AreaSelection | null = null;
let callbacks: StartAreaSelectionOptions | undefined;

function toAreaSelection(
  region: OverlayRegion,
  status: AreaSelection['status']
): AreaSelection {
  return {
    status,
    x: region.rect.x,
    y: region.rect.y,
    width: region.rect.width,
    height: region.rect.height,
  };
}

export function updateAreaSelectionCallbacks(
  options: StartAreaSelectionOptions
): void {
  callbacks = options;
}

async function resolvePreset(
  options?: StartAreaSelectionOptions
): Promise<Rectangle | null | undefined> {
  const mode = options?.mode ?? 'manual';

  if (mode === 'manual') {
    return options?.preset;
  }

  if (mode === 'window') {
    const windowSelection = await selectWindow();

    if (windowSelection.status === 'cancelled') {
      return null;
    }

    if (windowSelection.status === 'error' || !windowSelection.bounds) {
      console.error('Window selection failed:', windowSelection);
      return null;
    }

    return isWindows
      ? screen.screenToDipRect(null, windowSelection.bounds)
      : windowSelection.bounds;
  }

  if (screen.getAllDisplays().length === 1) {
    return screen.getPrimaryDisplay().bounds;
  }

  const displaySelection = await selectDisplay();
  const display = displayFromSelection(displaySelection);

  return display ? display.bounds : null;
}

export async function startAreaSelection(
  options?: StartAreaSelectionOptions
): Promise<AreaSelection | null> {
  const preset = await resolvePreset(options);

  if (preset === null || isOverlayActive()) {
    return null;
  }

  callbacks = options;
  pendingAreaSelection = null;

  const selection = await startInteractiveOverlay({
    preset,
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

  return toAreaSelection(selection, 'confirmed');
}

export async function confirmAreaSelection(): Promise<AreaSelection | null> {
  if (!pendingAreaSelection) {
    return null;
  }

  const selection: AreaSelection = {
    ...pendingAreaSelection,
    status: 'confirmed',
  };
  pendingAreaSelection = null;
  confirmOverlaySelection();

  return selection;
}

export async function cancelAreaSelection(
  silent: boolean = false
): Promise<void> {
  pendingAreaSelection = null;
  cancelOverlaySelection(silent);
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

export async function setAreaSelectorAspectRatio(
  ratio: AspectRatio
): Promise<void> {
  const value =
    ratio.width > 0 && ratio.height > 0 ? ratio.width / ratio.height : null;

  setOverlayAspectRatio(value);
}
