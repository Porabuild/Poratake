import type { NumberSize } from '@/types/editor';
import type { Rect } from '@/types/geometry';

export const NUMBER_SIZE_CONFIG: Record<
  NumberSize,
  { radius: number; fontSize: number }
> = {
  small: { radius: 14, fontSize: 14 },
  medium: { radius: 18, fontSize: 18 },
  large: { radius: 24, fontSize: 24 },
};

const ARROW_HEAD_ANGLE = Math.PI / 6;
const ARROW_CURVE_RATIO = 0.2;
const ARROW_BEND_THRESHOLD = 1;

export function arrowHeadSize(strokeWidth: number): number {
  return Math.max(16, strokeWidth * 5);
}

export function hasArrowBend(bendOffset?: { x: number; y: number }): boolean {
  return Boolean(
    bendOffset &&
    (Math.abs(bendOffset.x) > ARROW_BEND_THRESHOLD ||
      Math.abs(bendOffset.y) > ARROW_BEND_THRESHOLD)
  );
}

interface ArrowHeadPoints {
  leftX: number;
  leftY: number;
  rightX: number;
  rightY: number;
}

export function arrowHeadPoints(
  tipX: number,
  tipY: number,
  angle: number,
  headSize: number
): ArrowHeadPoints {
  return {
    leftX: tipX - headSize * Math.cos(angle - ARROW_HEAD_ANGLE),
    leftY: tipY - headSize * Math.sin(angle - ARROW_HEAD_ANGLE),
    rightX: tipX - headSize * Math.cos(angle + ARROW_HEAD_ANGLE),
    rightY: tipY - headSize * Math.sin(angle + ARROW_HEAD_ANGLE),
  };
}

export function curvedControlPoint(
  x1: number,
  y1: number,
  x2: number,
  y2: number
): { x: number; y: number } {
  const distance = Math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2);
  const curveOffset = distance * ARROW_CURVE_RATIO;
  const perpX = -(y2 - y1) / (distance || 1);
  const perpY = (x2 - x1) / (distance || 1);

  return {
    x: (x1 + x2) / 2 + perpX * curveOffset,
    y: (y1 + y2) / 2 + perpY * curveOffset,
  };
}

export function pointsToCoordinates(points: number[]): [number, number][] {
  const coords: [number, number][] = [];
  for (let i = 0; i < points.length; i += 2) {
    coords.push([points[i], points[i + 1]]);
  }
  return coords;
}

export function normalizeNegativeRect(
  x: number,
  y: number,
  width: number,
  height: number
): Rect {
  return {
    x: width < 0 ? x + width : x,
    y: height < 0 ? y + height : y,
    width: Math.abs(width),
    height: Math.abs(height),
  };
}
