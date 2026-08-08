export interface AreaOverlayRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AreaOverlayToolbar {
  kind: 'all-in-one';
  recordingEnabled: boolean;
}

export type AreaOverlayToolbarAction =
  | { action: 'close' }
  | { action: 'screenshot' }
  | { action: 'record' }
  | {
      action: 'select-aspect-ratio';
      name: string;
      width: number;
      height: number;
    }
  | { action: 'update-size'; width: number; height: number }
  | { action: 'size-editor-opened' }
  | { action: 'size-editor-closed' };

export interface AreaOverlayParams {
  displayId: number;
  imageUrl: string | null;
  interactive: boolean;
  showPrompt: boolean;
  rect: AreaOverlayRect | null;
  aspectRatio: number | null;
  toolbar: AreaOverlayToolbar | null;
}

export interface AreaOverlayResult extends AreaOverlayRect {
  displayId: number;
}

export interface AreaOverlayRectMessage {
  rect: AreaOverlayRect | null;
}

export interface AreaOverlayAspectRatioMessage {
  aspectRatio: number | null;
}

export interface AreaOverlayToolbarMessage {
  toolbar: AreaOverlayToolbar | null;
}
