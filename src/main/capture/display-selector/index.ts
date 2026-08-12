import { screen } from 'electron';
import type { Display, Rectangle } from 'electron';
import { daemon } from '@/main/daemon';
import { isWindows } from '@/main/utils/platform';

export interface DisplaySelection {
  status: 'selected' | 'cancelled';
  displayNumber?: number;
  screenId?: number;
  bounds?: Rectangle;
}

export function displayFromSelection(
  selection: DisplaySelection
): Display | null {
  if (selection.status !== 'selected' || !selection.bounds) {
    return null;
  }

  const dipBounds = isWindows
    ? screen.screenToDipRect(null, selection.bounds)
    : selection.bounds;

  return screen.getDisplayMatching(dipBounds);
}

let isSelecting = false;

export async function selectDisplay(): Promise<DisplaySelection> {
  if (isSelecting) {
    throw new Error('Display selector is already active');
  }

  isSelecting = true;

  try {
    const result = await daemon.call<DisplaySelection>(
      'display-selector',
      'select'
    );
    return result;
  } finally {
    isSelecting = false;
  }
}

export function killDisplaySelector(): void {
  if (isSelecting) {
    daemon.call('display-selector', 'cancel').catch(() => {});
    isSelecting = false;
  }
}
