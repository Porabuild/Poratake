export interface AreaSelection {
  status: 'ok' | 'cancelled' | 'selected' | 'updated' | 'confirmed';
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  screenId?: number;
}
