import { isWindows } from '@/main/utils/platform';
import * as daemonBackend from './daemon-backend';
import * as overlayBackend from './overlay-backend';
import type { AreaSelectorBackend } from './types';

export type {
  AreaSelectionCallback,
  AreaSelectionMode,
  AreaSelectionStyle,
  PresetArea,
  StartAreaSelectionOptions,
} from './types';

const backend: AreaSelectorBackend = isWindows ? overlayBackend : daemonBackend;

export const startAreaSelection = backend.startAreaSelection;
export const updateAreaSelectionCallbacks =
  backend.updateAreaSelectionCallbacks;
export const confirmAreaSelection = backend.confirmAreaSelection;
export const cancelAreaSelection = backend.cancelAreaSelection;
export const hasPendingSelection = backend.hasPendingSelection;
export const hideAreaSelector = backend.hideAreaSelector;
export const showAreaSelector = backend.showAreaSelector;
export const updateAreaSelection = backend.updateAreaSelection;
export const setAreaSelectorAspectRatio = backend.setAreaSelectorAspectRatio;

export function killAreaSelector(): Promise<void> {
  return backend.cancelAreaSelection(true);
}
