export interface ScrollCaptureArea {
  x: number;
  y: number;
  width: number;
  height: number;
  displayId: number;
}

export interface ScrollCaptureOverlayParams {
  displayId: number;
  area: ScrollCaptureArea;
  displayBounds: { x: number; y: number; width: number; height: number };
}

export interface ScrollCaptureControlState {
  isAutoScrolling: boolean;
  cursorOutside: boolean;
  frameCount: number;
  estimatedHeight: number;
}

export interface ScrollCaptureOverlayState extends ScrollCaptureControlState {
  preview: string | null;
  previewWidth: number | null;
  previewHeight: number | null;
}

export type ScrollCaptureAction = 'toggle-auto-scroll' | 'done' | 'cancel';

export const EMPTY_SCROLL_CAPTURE_STATE: ScrollCaptureOverlayState = {
  isAutoScrolling: false,
  cursorOutside: false,
  frameCount: 0,
  estimatedHeight: 0,
  preview: null,
  previewWidth: null,
  previewHeight: null,
};
