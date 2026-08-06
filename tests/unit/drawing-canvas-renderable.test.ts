import { describe, expect, it } from 'vitest';
import { getRenderableAnnotations } from '../../src/renderer/components/video-editor/composition/drawing-canvas-renderer';
import type { DrawingSegment } from '../../src/types/drawing';
import type { Annotation } from '../../src/types/editor';

const redact: Annotation = {
  id: 'r1',
  type: 'redact',
  x: 0,
  y: 0,
  width: 10,
  height: 10,
  style: 'pixelate',
  intensity: 5,
};

const rectangle: Annotation = {
  id: 'rect1',
  type: 'rectangle',
  x: 0,
  y: 0,
  width: 10,
  height: 10,
  stroke: '#fff',
  strokeWidth: 2,
};

function makeSegment(annotations: Annotation[]): DrawingSegment {
  return {
    id: 's1',
    startTime: 1,
    endTime: 5,
    canvasWidth: 100,
    canvasHeight: 100,
    annotations,
  };
}

describe('getRenderableAnnotations', () => {
  it('returns all annotations when not in redact-only mode', () => {
    const segment = makeSegment([rectangle, redact]);
    expect(getRenderableAnnotations(segment, 3, false)).toEqual([
      rectangle,
      redact,
    ]);
  });

  it('returns only redact annotations in redact-only mode', () => {
    const segment = makeSegment([rectangle, redact]);
    expect(getRenderableAnnotations(segment, 3, true)).toEqual([redact]);
  });

  it('returns nothing before the segment starts', () => {
    const segment = makeSegment([redact]);
    expect(getRenderableAnnotations(segment, 0.5, false)).toEqual([]);
  });

  it('returns nothing after the segment ends', () => {
    const segment = makeSegment([redact]);
    expect(getRenderableAnnotations(segment, 6, false)).toEqual([]);
  });

  it('includes annotations exactly at the segment boundaries', () => {
    const segment = makeSegment([redact]);
    expect(getRenderableAnnotations(segment, 1, false)).toEqual([redact]);
    expect(getRenderableAnnotations(segment, 5, false)).toEqual([redact]);
  });
});
