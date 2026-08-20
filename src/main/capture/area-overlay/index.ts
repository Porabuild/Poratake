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
  setOverlayFreeze,
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

export async function captureAreaToFile(
  filePath: string,
  onCaptureStarted?: () => void
): Promise<boolean> {
  const freeze = isFreezeScreenEnabled();
  const selection = await selectAreaWithOverlay({ freeze, interactive: true });
  if (!selection) {
    return false;
  }

  try {
    const capture = captureRegionToFile(selection.rect, filePath, {
      cached: selection.frozen,
    });
    onCaptureStarted?.();
    return await capture;
  } finally {
    await selection.release();
  }
}

export async function captureWindowToFile(
  filePath: string,
  onCaptureStarted?: () => void
): Promise<boolean> {
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
      const capture = captureWindowByIdToFile(selection.pickId, filePath);
      onCaptureStarted?.();
      return await capture;
    }

    const captureRect = pickTargets.captureRects.get(selection.pickId);
    if (!captureRect) {
      return false;
    }

    const capture = captureFrozenWindowToFile(
      captureRect,
      filePath,
      selection.pickId
    );
    onCaptureStarted?.();
    return await capture;
  } finally {
    await selection.release();
  }
}
