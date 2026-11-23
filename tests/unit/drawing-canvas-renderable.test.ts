import { describe, expect, it, vi } from 'vitest';
import {
  getRenderableAnnotations,
  renderDrawings,
} from '../../src/renderer/components/video-editor/composition/drawing-canvas-renderer';
import type { Context2D } from '../../src/renderer/components/video-editor/composition/types';
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

describe('renderDrawings draft redaction preview', () => {
  it('renders a draft blackout annotation even with no committed segments', () => {
    const fillRect = vi.fn();
    const ctx = {
      save: vi.fn(),
      restore: vi.fn(),
      fillStyle: '',
      fillRect,
    } as unknown as Context2D;
    const draftBlackout: Annotation = {
      id: 'draft-r',
      type: 'redact',
      x: 20,
      y: 30,
      width: -10,
      height: -15,
      style: 'blackout',
      intensity: 5,
    };

    renderDrawings(ctx, {
      drawingSegments: [],
      draftAnnotation: draftBlackout,
      timelineTime: 0,
      width: 100,
      height: 100,
    });

    expect(fillRect).toHaveBeenCalledExactlyOnceWith(10, 15, 10, 15);
  });

  it('skips non-redact draft annotations in redact-only mode', () => {
    const stroke = vi.fn();
    const ctx = {
      save: vi.fn(),
      restore: vi.fn(),
      beginPath: vi.fn(),
      roundRect: vi.fn(),
      stroke,
      fill: vi.fn(),
    } as unknown as Context2D;

    renderDrawings(ctx, {
      drawingSegments: [],
      draftAnnotation: rectangle,
      timelineTime: 0,
      width: 100,
      height: 100,
      onlyRedact: true,
    });

    expect(stroke).not.toHaveBeenCalled();
  });
});
