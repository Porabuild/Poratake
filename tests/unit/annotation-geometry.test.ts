import { describe, expect, it } from 'vitest';
import {
  NUMBER_SIZE_CONFIG,
  arrowHeadPoints,
  arrowHeadSize,
  curvedControlPoint,
  hasArrowBend,
} from '@/renderer/utils/annotation-geometry';

describe('annotation geometry', () => {
  it('keeps a minimum arrow head size for thin strokes', () => {
    expect(arrowHeadSize(1)).toBe(16);
    expect(arrowHeadSize(3)).toBe(16);
    expect(arrowHeadSize(4)).toBe(20);
    expect(arrowHeadSize(10)).toBe(50);
  });

  it('treats sub-pixel bend offsets as no bend', () => {
    expect(hasArrowBend(undefined)).toBe(false);
    expect(hasArrowBend({ x: 0, y: 0 })).toBe(false);
    expect(hasArrowBend({ x: 1, y: -1 })).toBe(false);
    expect(hasArrowBend({ x: 1.5, y: 0 })).toBe(true);
    expect(hasArrowBend({ x: 0, y: -2 })).toBe(true);
  });

  it('places arrow head points symmetrically behind the tip', () => {
    const points = arrowHeadPoints(100, 0, 0, 20);

    expect(points.leftX).toBeCloseTo(100 - 20 * Math.cos(-Math.PI / 6));
    expect(points.leftY).toBeCloseTo(-20 * Math.sin(-Math.PI / 6));
    expect(points.rightX).toBeCloseTo(points.leftX);
    expect(points.rightY).toBeCloseTo(-points.leftY);
  });

  it('offsets the curve control point perpendicular to the line', () => {
    const control = curvedControlPoint(0, 0, 100, 0);

    expect(control.x).toBeCloseTo(50);
    expect(control.y).toBeCloseTo(20);
  });

  it('does not divide by zero for a zero-length line', () => {
    const control = curvedControlPoint(10, 10, 10, 10);

    expect(Number.isFinite(control.x)).toBe(true);
    expect(Number.isFinite(control.y)).toBe(true);
    expect(control).toEqual({ x: 10, y: 10 });
  });

  it('exposes one number size table for both renderers', () => {
    expect(NUMBER_SIZE_CONFIG.small).toEqual({ radius: 14, fontSize: 14 });
    expect(NUMBER_SIZE_CONFIG.medium).toEqual({ radius: 18, fontSize: 18 });
    expect(NUMBER_SIZE_CONFIG.large).toEqual({ radius: 24, fontSize: 24 });
  });
});
