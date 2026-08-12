import type { AreaSelection } from '@/types/area';
import type {
  AreaOverlayToolbar,
  AreaOverlayToolbarAction,
} from '@/types/area-overlay';
import type { AspectRatio } from '@/types/aspect-ratio';

export type AreaSelectionCallback = (selection: AreaSelection) => void;

export type AreaSelectionMode = 'manual' | 'display' | 'window';

export type AreaSelectionStyle = 'default' | 'simple';

export interface PresetArea {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface StartAreaSelectionOptions {
  mode?: AreaSelectionMode;
  freeze?: boolean;
  visible?: boolean;
  preset?: PresetArea;
  onUpdate?: AreaSelectionCallback;
  onSelected?: AreaSelectionCallback;
  onCancelled?: () => void;
  onToolbarAction?: (action: AreaOverlayToolbarAction) => void;
  showPrompt?: boolean;
  style?: AreaSelectionStyle;
  toolbar?: AreaOverlayToolbar;
}

export interface ConfirmAreaSelectionOptions {
  keepOverlayVisible?: boolean;
}

export interface AreaSelectorBackend {
  startAreaSelection(
    options?: StartAreaSelectionOptions
  ): Promise<AreaSelection | null>;
  updateAreaSelectionCallbacks(options: StartAreaSelectionOptions): void;
  confirmAreaSelection(
    options?: ConfirmAreaSelectionOptions
  ): Promise<AreaSelection | null>;
  concealAreaSelectorOverlay(): void;
  hasVisibleSelectorOverlay(): boolean;
  cancelAreaSelection(silent?: boolean): Promise<void>;
  hasPendingSelection(): boolean;
  hideAreaSelector(): Promise<void>;
  showAreaSelector(): Promise<void>;
  updateAreaSelection(bounds: PresetArea): Promise<boolean>;
  setAreaSelectionMode(mode: AreaSelectionMode): Promise<void>;
  setAreaSelectorAspectRatio(ratio: AspectRatio): Promise<void>;
}
