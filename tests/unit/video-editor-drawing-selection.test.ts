import { describe, expect, it } from 'vitest';
import { mapAnnotationIdsToSegmentIds } from '../../src/renderer/components/video-editor/utils';

describe('mapAnnotationIdsToSegmentIds', () => {
  it('maps annotation ids to their owning segment ids', () => {
    const map = new Map([
      ['a1', 's1'],
      ['a2', 's2'],
    ]);

    expect(mapAnnotationIdsToSegmentIds(['a1', 'a2'], map)).toEqual([
      's1',
      's2',
    ]);
  });

  it('deduplicates segment ids while preserving first-seen order', () => {
    const map = new Map([
      ['a1', 's1'],
      ['a2', 's1'],
      ['a3', 's2'],
    ]);

    expect(mapAnnotationIdsToSegmentIds(['a3', 'a1', 'a2'], map)).toEqual([
      's2',
      's1',
    ]);
  });

  it('ignores annotation ids without a known segment', () => {
    const map = new Map([['a1', 's1']]);

    expect(mapAnnotationIdsToSegmentIds(['a1', 'unknown'], map)).toEqual([
      's1',
    ]);
  });

  it('returns an empty array when no annotations are provided', () => {
    expect(mapAnnotationIdsToSegmentIds([], new Map())).toEqual([]);
  });
});
