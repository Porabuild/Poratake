import { cancelAreaSelection } from './overlay-backend';

export type {
  AreaSelectionCallback,
  AreaSelectionMode,
  ConfirmAreaSelectionOptions,
  PresetArea,
  StartAreaSelectionOptions,
} from './types';

export {
  startAreaSelection,
  updateAreaSelectionCallbacks,
  confirmAreaSelection,
  concealAreaSelectorOverlay,
  setAreaSelectorFreeze,
  hasVisibleSelectorOverlay,
  cancelAreaSelection,
  hasPendingSelection,
  hideAreaSelector,
  showAreaSelector,
  updateAreaSelection,
  setAreaSelectionMode,
  setAreaSelectorAspectRatio,
} from './overlay-backend';

export function killAreaSelector(): Promise<void> {
  return cancelAreaSelection(true);
}
