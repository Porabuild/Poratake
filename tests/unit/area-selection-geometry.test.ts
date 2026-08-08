import { describe, it, expect } from 'vitest';
import {
  adjustRectToRatio,
  containsPoint,
  cursorFor,
  fitRect,
  hitTestHandle,
  isUsableSelection,
  moveRect,
  normalizeRect,
  resizeRect,
} from '@/renderer/utils/area-selection';

const rect = { x: 100, y: 100, width: 400, height: 200 };
const bounds = { width: 1920, height: 1080 };

describe('area selection geometry', () => {
  it('normalizes a drag in any direction', () => {
    expect(normalizeRect({ x: 300, y: 250 }, { x: 100, y: 50 })).toEqual({
      x: 100,
      y: 50,
      width: 200,
      height: 200,
    });
  });

  it('rejects selections at or below the minimum drag size', () => {
    expect(isUsableSelection({ x: 0, y: 0, width: 10, height: 40 })).toBe(
      false
    );
    expect(isUsableSelection({ x: 0, y: 0, width: 11, height: 40 })).toBe(true);
  });

  it('detects corner and edge handles before the interior', () => {
    expect(hitTestHandle(rect, { x: 100, y: 100 })).toBe('top-left');
    expect(hitTestHandle(rect, { x: 500, y: 300 })).toBe('bottom-right');
    expect(hitTestHandle(rect, { x: 300, y: 100 })).toBe('top');
    expect(hitTestHandle(rect, { x: 100, y: 200 })).toBe('left');
    expect(hitTestHandle(rect, { x: 300, y: 200 })).toBeNull();
  });

  it('maps pointer position to a cursor', () => {
    expect(cursorFor(null, { x: 10, y: 10 })).toBe('crosshair');
    expect(cursorFor(rect, { x: 100, y: 100 })).toBe('nwse-resize');
    expect(cursorFor(rect, { x: 300, y: 100 })).toBe('ns-resize');
    expect(cursorFor(rect, { x: 300, y: 200 })).toBe('move');
    expect(cursorFor(rect, { x: 900, y: 900 })).toBe('crosshair');
  });

  it('keeps the opposite edge fixed while resizing', () => {
    expect(resizeRect(rect, { x: 60, y: 60 }, 'top-left', null)).toEqual({
      x: 60,
      y: 60,
      width: 440,
      height: 240,
    });
  });

  it('enforces the minimum size when a handle crosses over', () => {
    expect(resizeRect(rect, { x: 900, y: 200 }, 'left', null)).toEqual({
      x: 480,
      y: 100,
      width: 20,
      height: 200,
    });
  });

  it('anchors a ratio change to the dragged handle', () => {
    expect(adjustRectToRatio(rect, 1, 'left')).toEqual({
      x: 300,
      y: 100,
      width: 200,
      height: 200,
    });
  });

  it('anchors a ratio change to the centre when no handle is dragged', () => {
    expect(adjustRectToRatio(rect, 1, null)).toEqual({
      x: 200,
      y: 100,
      width: 200,
      height: 200,
    });
  });

  it('grows the shorter side when the rect is taller than the ratio', () => {
    expect(
      adjustRectToRatio({ x: 0, y: 0, width: 100, height: 400 }, 1, 'bottom')
    ).toEqual({ x: 0, y: 0, width: 100, height: 100 });
  });

  it('clamps a moved rect inside the display', () => {
    expect(
      moveRect(rect, { x: 5000, y: 5000 }, { x: 10, y: 10 }, bounds)
    ).toEqual({ x: 1520, y: 880, width: 400, height: 200 });
  });

  it('pins an oversized rect to the origin while moving', () => {
    const oversized = { x: 0, y: 0, width: 4000, height: 4000 };
    expect(
      moveRect(oversized, { x: 50, y: 50 }, { x: 0, y: 0 }, bounds)
    ).toEqual(oversized);
  });

  it('fits a rect that overflows the display', () => {
    expect(
      fitRect({ x: 1800, y: 1000, width: 400, height: 400 }, bounds)
    ).toEqual({ x: 1520, y: 680, width: 400, height: 400 });
  });

  it('reports interior hits', () => {
    expect(containsPoint(rect, { x: 200, y: 150 })).toBe(true);
    expect(containsPoint(rect, { x: 200, y: 350 })).toBe(false);
  });
});
