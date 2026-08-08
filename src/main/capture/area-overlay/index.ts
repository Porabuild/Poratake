import { captureRegionToFile } from '@/main/capture/screenshot/native-capture';
import { isFreezeScreenEnabled } from '@/main/capture/freeze-screen/preference';
import { startOverlaySession } from './session';
import type { OverlayOptions, OverlaySelection } from './session';

export type {
  OverlayCallbacks,
  OverlayOptions,
  OverlayRegion,
  OverlaySelection,
} from './session';

export {
  cancelOverlaySelection,
  confirmOverlaySelection,
  isOverlayActive,
  setOverlayAspectRatio,
  setOverlayToolbar,
  setOverlayVisible,
  updateOverlaySelection,
} from './session';

export function selectAreaWithOverlay(
  options?: OverlayOptions
): Promise<OverlaySelection | null> {
  return startOverlaySession(options);
}

export function startInteractiveOverlay(
  options?: Omit<OverlayOptions, 'interactive' | 'freeze'>
): Promise<OverlaySelection | null> {
  return startOverlaySession({ ...options, interactive: true, freeze: false });
}

export async function captureAreaToFile(filePath: string): Promise<boolean> {
  const freeze = isFreezeScreenEnabled();
  const selection = await selectAreaWithOverlay({ freeze });
  if (!selection) {
    return false;
  }

  try {
    return await captureRegionToFile(selection.rect, filePath, {
      cached: freeze,
    });
  } finally {
    await selection.release();
  }
}
