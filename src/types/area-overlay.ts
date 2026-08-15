export interface AreaOverlayRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AreaOverlayPickTarget extends AreaOverlayRect {
  id: number;
}

export interface AreaOverlayToolbar {
  kind: 'all-in-one';
  recordingEnabled: boolean;
  ocrEnabled: boolean;
  activeMode: AllInOneCaptureMode;
  activeTarget: AllInOneCaptureTarget;
}

export type AllInOneCaptureMode = 'screenshot' | 'record' | 'ocr';

export type AllInOneCaptureTarget = 'area' | 'window' | 'screen';

export type AreaOverlayRenderer = 'area-overlay' | 'scroll-capture-overlay';

export type AreaOverlayToolbarAction =
  | { action: 'close' }
  | { action: 'copy-color'; color: string }
  | { action: 'select-capture-mode'; mode: AllInOneCaptureMode }
  | { action: 'select-capture-target'; target: AllInOneCaptureTarget };

export interface AreaOverlayParams {
  sessionId: number;
  displayId: number;
  imageUrl: string | null;
  interactive: boolean;
  autoConfirm: boolean;
  repeatablePicks: boolean;
  showPrompt: boolean;
  rect: AreaOverlayRect | null;
  aspectRatio: number | null;
  toolbar: AreaOverlayToolbar | null;
  pickTargets: AreaOverlayPickTarget[] | null;
  prompt: string | null;
}

export interface AreaOverlayResult extends AreaOverlayRect {
  displayId: number;
  pickId?: number;
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

export interface AreaOverlayPickTargetsMessage {
  pickTargets: AreaOverlayPickTarget[] | null;
  prompt: string | null;
  repeatablePicks: boolean;
}
