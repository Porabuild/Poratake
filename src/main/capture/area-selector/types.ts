import type { AreaSelection } from '@/types/area';
import type {
  AreaOverlayRenderer,
  AreaOverlayToolbar,
  AreaOverlayToolbarAction,
} from '@/types/area-overlay';

export type AreaSelectionCallback = (selection: AreaSelection) => void;

export type AreaSelectionMode = 'manual' | 'display' | 'window';

export interface PresetArea {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface StartAreaSelectionOptions {
  mode?: AreaSelectionMode;
  requireDisplayPick?: boolean;
  freeze?: boolean;
  visible?: boolean;
  preset?: PresetArea;
  onUpdate?: AreaSelectionCallback;
  onSelected?: AreaSelectionCallback;
  onCancelled?: () => void;
  onToolbarAction?: (action: AreaOverlayToolbarAction) => void;
  showPrompt?: boolean;
  renderer?: AreaOverlayRenderer;
  toolbar?: AreaOverlayToolbar;
}

export interface ConfirmAreaSelectionOptions {
  keepOverlayVisible?: boolean;
}
