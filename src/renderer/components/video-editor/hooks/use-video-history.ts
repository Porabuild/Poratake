import { useCallback } from 'react';
import type { Segment } from '../types';
import type { SliceController } from './use-editor-history';

type SegmentUpdater = Segment[] | ((prev: Segment[]) => Segment[]);

interface UseVideoHistoryReturn {
  segments: Segment[];
  setSegments: (updater: SegmentUpdater) => void;
  setSegmentsWithoutHistory: (updater: SegmentUpdater) => void;
  commitSegmentsToHistory: () => void;
}

export function useVideoHistory(
  slice: SliceController<Segment[]>
): UseVideoHistoryReturn {
  const { value: segments, set, setWithoutHistory, commit } = slice;

  const setSegments = useCallback(
    (updater: SegmentUpdater) => set(updater),
    [set]
  );

  const setSegmentsWithoutHistory = useCallback(
    (updater: SegmentUpdater) => setWithoutHistory(updater),
    [setWithoutHistory]
  );

  const commitSegmentsToHistory = useCallback(() => commit(), [commit]);

  return {
    segments,
    setSegments,
    setSegmentsWithoutHistory,
    commitSegmentsToHistory,
  };
}
