import { cancelAreaSelection } from './overlay-backend';

export type {
  AreaSelectionCallback,
  AreaSelectionMode,
  ConfirmAreaSelectionOptions,
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
  suspendAreaSelector,
  updateAreaSelection,
  setAreaSelectionMode,
  setAreaSelectorAspectRatio,
  setAreaSelectorToolbar,
} from './overlay-backend';

export function killAreaSelector(): Promise<void> {
  return cancelAreaSelection(true);
}
