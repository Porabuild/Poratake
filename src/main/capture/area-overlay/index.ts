import {
  captureFrozenWindowToFile,
  captureRegionToFile,
  captureWindowByIdToFile,
} from '@/main/capture/screenshot/native-capture';
import { isFreezeScreenEnabled } from '@/main/capture/freeze-screen/preference';
import { startOverlaySession } from './session';
import { resolveWindowPickTargets } from './window-pick-targets';
import type { OverlayOptions, OverlaySelection } from './session';

export type {
  OverlayCallbacks,
  OverlayOptions,
  OverlayPickTarget,
  OverlayRegion,
  OverlaySelection,
} from './session';
export type { WindowPickTargets } from './window-pick-targets';

export {
  cancelOverlaySelection,
  concealOverlayHandoff,
  confirmOverlaySelection,
  getActiveOverlayWindowAtPoint,
  getOverlayWindowIds,
  hasOverlayHandoff,
  isOverlayActive,
  prewarmAreaOverlay,
  retainOverlayHandoffWindow,
  setOverlayAspectRatio,
  setOverlayPickTargets,
  setOverlayToolbar,
  setOverlayVisible,
  updateOverlaySelection,
} from './session';

export { resolveWindowPickTargets } from './window-pick-targets';

export function selectAreaWithOverlay(
  options?: OverlayOptions
): Promise<OverlaySelection | null> {
  return startOverlaySession(options);
}

export function startInteractiveOverlay(
  options?: Omit<OverlayOptions, 'interactive'>
): Promise<OverlaySelection | null> {
  return startOverlaySession({
    ...options,
    interactive: true,
    freeze: options?.freeze ?? false,
  });
}

export async function captureAreaToFile(filePath: string): Promise<boolean> {
  const freeze = isFreezeScreenEnabled();
  const selection = await selectAreaWithOverlay({ freeze, interactive: true });
  if (!selection) {
    return false;
  }

  try {
    return await captureRegionToFile(selection.rect, filePath, {
      cached: selection.frozen,
    });
  } finally {
    await selection.release();
  }
}

export async function captureWindowToFile(filePath: string): Promise<boolean> {
  const pickTargets = await resolveWindowPickTargets();
  if (!pickTargets) {
    return false;
  }

  const selection = await selectAreaWithOverlay({
    freeze: isFreezeScreenEnabled(),
    interactive: true,
    visible: true,
    pickTargets: pickTargets.targets,
    prompt: pickTargets.prompt,
  });
  if (!selection) {
    return false;
  }

  try {
    if (selection.pickId === undefined) {
      return false;
    }

    if (!selection.frozen) {
      return await captureWindowByIdToFile(selection.pickId, filePath);
    }

    const captureRect = pickTargets.captureRects.get(selection.pickId);
    if (!captureRect) {
      return false;
    }

    return await captureFrozenWindowToFile(
      captureRect,
      filePath,
      selection.pickId
    );
  } finally {
    await selection.release();
  }
}
