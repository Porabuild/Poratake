import type { Rect } from './geometry';

export interface AreaSelection {
  status: 'cancelled' | 'selected' | 'updated' | 'confirmed';
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  screenId?: number;
  windowId?: number;
  windowName?: string;
  windowBounds?: Rect;
}
