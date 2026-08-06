import type { RefObject } from 'react';

export interface TimelineContextValue {
  pixelsPerSecond: number;
  scrollContainerRef: RefObject<HTMLDivElement>;
  timeToPixels: (time: number) => number;
  pixelsToTime: (pixels: number) => number;
  zoomIn: () => void;
  zoomOut: () => void;
  setZoomLevel: (pixels: number) => void;
  resetZoom: () => void;
  canZoomIn: boolean;
  canZoomOut: boolean;
}
