import { describe, it, expect } from 'vitest';
import { getSegmentBoundaryTransition } from '../../src/renderer/components/video-editor/utils';
import type { Segment } from '../../src/renderer/components/video-editor/types';

function createSegment(id: string, start: number, end: number): Segment {
  return {
    id,
    originalStart: start,
    originalEnd: end,
    trimMinStart: start,
    trimMaxEnd: end,
  };
}

describe('getSegmentBoundaryTransition', () => {
  it('marks empty segment lists as final', () => {
    const result = getSegmentBoundaryTransition([], 0);

    expect(result.isFinalSegment).toBe(true);
    expect(result.nextSegmentIndex).toBeNull();
  });

  it('returns next segment index when there is a following segment', () => {
    const segments = [
      createSegment('1', 0, 2),
      createSegment('2', 2, 4),
      createSegment('3', 4, 6),
    ];

    const result = getSegmentBoundaryTransition(segments, 1);

    expect(result.isFinalSegment).toBe(false);
    expect(result.nextSegmentIndex).toBe(2);
  });

  it('marks the timeline segment tail as final after reorder', () => {
    const segments = [
      createSegment('3', 8, 12),
      createSegment('1', 0, 4),
      createSegment('2', 4, 8),
    ];

    const result = getSegmentBoundaryTransition(segments, 0);

    expect(result.isFinalSegment).toBe(false);
    expect(result.nextSegmentIndex).toBe(1);
  });

  it('marks last timeline segment as final', () => {
    const segments = [
      createSegment('2', 4, 8),
      createSegment('3', 8, 12),
      createSegment('1', 0, 4),
    ];

    const result = getSegmentBoundaryTransition(segments, 2);

    expect(result.isFinalSegment).toBe(true);
    expect(result.nextSegmentIndex).toBeNull();
  });
});
