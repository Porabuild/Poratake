export interface Point {
  x: number;
  y: number;
}

export interface Size {
  width: number;
  height: number;
}

export interface Rect extends Point, Size {}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}
