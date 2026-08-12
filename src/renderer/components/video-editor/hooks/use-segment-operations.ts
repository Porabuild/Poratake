import { useState, useCallback, useEffect, useRef } from 'react';
import type { Segment, TrimState, NativeVideoPlayerHandle } from '../types';
import { getSegmentDuration } from '../utils';
import { splitVideoSegments } from '../timeline-split';

interface UseSegmentOperationsProps {
  segments: Segment[];
  setSegments: (updater: Segment[] | ((prev: Segment[]) => Segment[])) => void;
  setSegmentsWithoutHistory: (
    updater: Segment[] | ((prev: Segment[]) => Segment[])
  ) => void;
  commitSegmentsToHistory: () => void;
  totalTimelineDuration: number;
  originalDuration: number;
  pixelsPerSecond: number;
  nativePlayerRef: React.RefObject<NativeVideoPlayerHandle | null>;
  timelineRef: React.RefObject<HTMLDivElement | null>;
  setTimelinePosition: (pos: number) => void;
  onTimelineRangesAdjust: (
    segmentIndex: number,
    oldSegmentDuration: number,
    newSegmentDuration: number,
    segmentStartOnTimeline: number,
    segmentEndOnTimeline: number,
    newTotalDuration: number,
    nextSegments: Segment[]
  ) => void;
}

interface UseSegmentOperationsReturn {
  selectedSegmentId: string | null;
  setSelectedSegmentId: (id: string | null) => void;
  isCutToolActive: boolean;
  trimState: TrimState | null;
  selectedSegmentSpeed: number;
  toggleCutTool: () => void;
  handleSegmentSelect: (segmentId: string | null) => void;
  handleDeleteSegment: () => void;
  handleTrimStart: (
    e: React.MouseEvent,
    segmentId: string,
    edge: 'start' | 'end'
  ) => void;
  handleCut: (cutVideoTime: number) => void;
  handleSpeedChange: (speed: number) => void;
  handleReorderSegment: (segmentId: string, newIndex: number) => void;
  clearSegmentSelection: () => void;
}

export function useSegmentOperations({
  segments,
  setSegments,
  setSegmentsWithoutHistory,
  commitSegmentsToHistory,
  totalTimelineDuration,
  originalDuration,
  pixelsPerSecond,
  nativePlayerRef,
  timelineRef,
  setTimelinePosition,
  onTimelineRangesAdjust,
}: UseSegmentOperationsProps): UseSegmentOperationsReturn {
  const [selectedSegmentId, setSelectedSegmentId] = useState<string | null>(
    null
  );
  const [isCutToolActive, setIsCutToolActive] = useState(false);
  const [trimState, setTrimState] = useState<TrimState | null>(null);
  const setCurrentSegmentIndexRef = useRef<(idx: number) => void>(() => {});

  const toggleCutTool = useCallback(() => {
    setIsCutToolActive(prev => !prev);
    setSelectedSegmentId(null);
  }, []);

  const handleSegmentSelect = useCallback(
    (segmentId: string | null) => {
      if (isCutToolActive) return;
      setSelectedSegmentId(segmentId);
    },
    [isCutToolActive]
  );

  const handleDeleteSegment = useCallback(() => {
    if (!selectedSegmentId || segments.length <= 1) return;

    const newSegments = segments.filter(s => s.id !== selectedSegmentId);
    setSegments(newSegments);
    setSelectedSegmentId(null);
    setTimelinePosition(0);
    setCurrentSegmentIndexRef.current(0);

    if (newSegments.length > 0) {
      nativePlayerRef.current?.seekTo(0);
    }
  }, [
    selectedSegmentId,
    segments,
    setSegments,
    setTimelinePosition,
    nativePlayerRef,
  ]);

  const handleTrimStart = useCallback(
    (e: React.MouseEvent, segmentId: string, edge: 'start' | 'end') => {
      e.stopPropagation();
      e.preventDefault();

      const segment = segments.find(s => s.id === segmentId);
      if (!segment) return;

      const scrollLeft = timelineRef.current?.scrollLeft ?? 0;

      setTrimState({
        segmentId,
        edge,
        initialMouseX: e.clientX,
        initialValue:
          edge === 'start' ? segment.originalStart : segment.originalEnd,
        initialTimelineDuration: totalTimelineDuration,
        initialScrollLeft: scrollLeft,
      });
    },
    [segments, totalTimelineDuration, timelineRef]
  );

  const handleTrimMove = useCallback(
    (e: MouseEvent) => {
      if (!trimState || !timelineRef.current || originalDuration === 0) return;

      const timeline = timelineRef.current;
      const currentScrollLeft = timeline.scrollLeft;
      const scrollDelta = currentScrollLeft - trimState.initialScrollLeft;
      const deltaX = e.clientX - trimState.initialMouseX + scrollDelta;

      setSegmentsWithoutHistory(prevSegments => {
        const segmentIndex = prevSegments.findIndex(
          (s: Segment) => s.id === trimState.segmentId
        );
        if (segmentIndex === -1) return prevSegments;

        const seg = prevSegments[segmentIndex];
        const speed = seg.speed ?? 1;
        const deltaTime = (deltaX / pixelsPerSecond) * speed;
        const minDuration = 0.5;

        if (trimState.edge === 'start') {
          const newStart = Math.max(
            seg.trimMinStart,
            Math.min(
              seg.originalEnd - minDuration,
              trimState.initialValue + deltaTime
            )
          );
          return prevSegments.map((s: Segment, i: number) =>
            i === segmentIndex ? { ...s, originalStart: newStart } : s
          );
        } else {
          const newEnd = Math.min(
            seg.trimMaxEnd,
            Math.max(
              seg.originalStart + minDuration,
              trimState.initialValue + deltaTime
            )
          );
          return prevSegments.map((s: Segment, i: number) =>
            i === segmentIndex ? { ...s, originalEnd: newEnd } : s
          );
        }
      });
    },
    [
      trimState,
      originalDuration,
      pixelsPerSecond,
      setSegmentsWithoutHistory,
      timelineRef,
    ]
  );

  const handleTrimEnd = useCallback(() => {
    if (!trimState) return;

    const segment = segments.find((s: Segment) => s.id === trimState.segmentId);
    if (segment) {
      let timelinePos = 0;
      for (const seg of segments) {
        if (seg.id === segment.id) break;
        timelinePos += getSegmentDuration(seg);
      }
      nativePlayerRef.current?.seekTo(timelinePos);
    }
    commitSegmentsToHistory();
    setTrimState(null);
  }, [trimState, segments, commitSegmentsToHistory, nativePlayerRef]);

  useEffect(() => {
    if (trimState) {
      const handleMouseMove = (e: MouseEvent) => handleTrimMove(e);
      const handleMouseUp = () => handleTrimEnd();

      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);

      return () => {
        document.removeEventListener('mousemove', handleMouseMove);
        document.removeEventListener('mouseup', handleMouseUp);
      };
    }
  }, [trimState, handleTrimMove, handleTrimEnd]);

  const handleCut = useCallback(
    (cutVideoTime: number) => {
      const newSegments = splitVideoSegments(segments, cutVideoTime);
      if (newSegments) setSegments(newSegments);
    },
    [segments, setSegments]
  );

  const handleSpeedChange = useCallback(
    (speed: number) => {
      if (!selectedSegmentId) return;

      const segmentIndex = segments.findIndex(s => s.id === selectedSegmentId);
      if (segmentIndex === -1) return;

      const segment = segments[segmentIndex];
      const oldSpeed = segment.speed ?? 1;
      const segmentVideoDuration = segment.originalEnd - segment.originalStart;
      const oldSegmentDuration = segmentVideoDuration / oldSpeed;
      const newSegmentDuration = segmentVideoDuration / speed;

      let segmentStartOnTimeline = 0;
      for (let i = 0; i < segmentIndex; i++) {
        const seg = segments[i];
        const spd = seg.speed ?? 1;
        segmentStartOnTimeline += (seg.originalEnd - seg.originalStart) / spd;
      }

      const segmentEndOnTimeline = segmentStartOnTimeline + oldSegmentDuration;
      const durationDelta = newSegmentDuration - oldSegmentDuration;
      const newTotalDuration = totalTimelineDuration + durationDelta;
      const nextSegments = segments.map(seg =>
        seg.id === selectedSegmentId ? { ...seg, speed } : seg
      );

      onTimelineRangesAdjust(
        segmentIndex,
        oldSegmentDuration,
        newSegmentDuration,
        segmentStartOnTimeline,
        segmentEndOnTimeline,
        newTotalDuration,
        nextSegments
      );
    },
    [selectedSegmentId, segments, totalTimelineDuration, onTimelineRangesAdjust]
  );

  const handleReorderSegment = useCallback(
    (segmentId: string, newIndex: number) => {
      const currentIndex = segments.findIndex(s => s.id === segmentId);
      if (currentIndex === -1) return;
      if (currentIndex === newIndex) return;
      if (newIndex < 0 || newIndex >= segments.length) return;

      const newSegments = [...segments];
      const [moved] = newSegments.splice(currentIndex, 1);
      newSegments.splice(newIndex, 0, moved);
      setSegments(newSegments);
      setSelectedSegmentId(segmentId);
    },
    [segments, setSegments]
  );

  const selectedSegmentSpeed = (() => {
    if (!selectedSegmentId) return 1;
    const segment = segments.find(s => s.id === selectedSegmentId);
    return segment?.speed ?? 1;
  })();

  const clearSegmentSelection = useCallback(() => {
    setSelectedSegmentId(null);
    setIsCutToolActive(false);
  }, []);

  return {
    selectedSegmentId,
    setSelectedSegmentId,
    isCutToolActive,
    trimState,
    selectedSegmentSpeed,
    toggleCutTool,
    handleSegmentSelect,
    handleDeleteSegment,
    handleTrimStart,
    handleCut,
    handleSpeedChange,
    handleReorderSegment,
    clearSegmentSelection,
  };
}
