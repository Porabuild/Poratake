import { describe, expect, it } from 'vitest';
import type { Annotation } from '../../src/types/editor';
import {
  inverseScaleAnnotationUpdates,
  scaleAnnotationToComposition,
} from '../../src/renderer/components/video-editor/composition/drawing-scale';

describe('scaleAnnotationToComposition', () => {
  it('scales rectangle position and size by axis factors', () => {
    const rect: Annotation = {
      id: 'r',
      type: 'rectangle',
      x: 10,
      y: 20,
      width: 30,
      height: 40,
      stroke: '#fff',
      strokeWidth: 4,
    };

    const scaled = scaleAnnotationToComposition(rect, 2, 3);

    expect(scaled).toMatchObject({
      x: 20,
      y: 60,
      width: 60,
      height: 120,
    });
  });

  it('scales pen points by alternating axis factors', () => {
    const pen: Annotation = {
      id: 'p',
      type: 'pen',
      points: [1, 2, 3, 4],
      stroke: '#fff',
      strokeWidth: 2,
    };

    const scaled = scaleAnnotationToComposition(pen, 2, 5);

    expect(scaled.type === 'pen' && scaled.points).toEqual([2, 10, 6, 20]);
  });
});

describe('inverseScaleAnnotationUpdates', () => {
  it('inverts a position update so a move in composition space maps back to stored space', () => {
    const updates = inverseScaleAnnotationUpdates({ x: 20, y: 60 }, 2, 3);
    expect(updates).toMatchObject({ x: 10, y: 20 });
  });

  it('inverts point updates by alternating axis factors', () => {
    const updates = inverseScaleAnnotationUpdates(
      { points: [2, 10, 6, 20] },
      2,
      5
    );
    expect(updates.points).toEqual([1, 2, 3, 4]);
  });

  it('round-trips a rectangle resize update', () => {
    const scaleX = 1.5;
    const scaleY = 2.5;
    const compositionUpdate = { x: 30, y: 50, width: 90, height: 100 };
    const stored = inverseScaleAnnotationUpdates(
      compositionUpdate,
      scaleX,
      scaleY
    );

    expect(stored).toMatchObject({
      x: compositionUpdate.x / scaleX,
      y: compositionUpdate.y / scaleY,
      width: compositionUpdate.width / scaleX,
      height: compositionUpdate.height / scaleY,
    });
  });

  it('inverts radius and stroke width by the averaged scale', () => {
    const updates = inverseScaleAnnotationUpdates(
      { radius: 20, strokeWidth: 8 },
      2,
      4
    );
    expect(updates).toMatchObject({ radius: 20 / 3, strokeWidth: 8 / 3 });
  });
});
