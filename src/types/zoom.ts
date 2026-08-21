export type ZoomTargetMode = 'cursor' | 'manual';

export interface ZoomFocusPoint {
  x: number;
  y: number;
}

export interface ZoomSegment {
  id: string;
  startTime: number;
  endTime: number;
  zoomLevel: number;
  transitionInDuration?: number;
  transitionOutDuration?: number;
  targetMode?: ZoomTargetMode;
  focusPoint?: ZoomFocusPoint;
}

export interface ZoomSettings {
  transitionInDuration: number;
  transitionOutDuration: number;
  easing: 'ease-in-out';
  followSmoothness: number;
  lookAhead: number;
}

export const DEFAULT_ZOOM_SETTINGS: ZoomSettings = {
  transitionInDuration: 1.2,
  transitionOutDuration: 1.2,
  easing: 'ease-in-out',
  followSmoothness: 0.3,
  lookAhead: 0.12,
};

export const DEFAULT_ZOOM_LEVEL = 1.2;

export const BOUNDING_RATIO = 0.5;

export const MIN_ZOOM_LEVEL = 1;
export const MAX_ZOOM_LEVEL = 3;
export const ZOOM_LEVEL_STEP = 0.25;

export const ZOOM_LEVELS = [
  { value: 1.25, label: '1.25x' },
  { value: 1.5, label: '1.5x' },
  { value: 1.75, label: '1.75x' },
  { value: 2, label: '2x' },
  { value: 2.25, label: '2.25x' },
  { value: 2.5, label: '2.5x' },
  { value: 2.75, label: '2.75x' },
  { value: 3, label: '3x' },
];
