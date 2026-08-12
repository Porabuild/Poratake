import { useCallback, useEffect, useRef, useState } from 'react';
import type { Annotation } from '@/types/editor';
import type { DrawingSegment } from '@/types/drawing';
import {
  DEFAULT_DRAWING_SEGMENT_DURATION,
  MIN_DRAWING_SEGMENT_DURATION,
} from '@/types/drawing';
import { splitDrawingSegment } from '../timeline-split';
import type { SliceController } from './use-editor-history';

interface UseDrawingSegmentsProps {
  totalTimelineDuration: number;
  slice: SliceController<DrawingSegment[]>;
}

interface CreateDrawingSegmentParams {
  annotation: Annotation;
  timelinePosition: number;
  canvasWidth: number;
  canvasHeight: number;
}

interface UseDrawingSegmentsReturn {
  drawingSegments: DrawingSegment[];
  setDrawingSegments: (
    updater: DrawingSegment[] | ((prev: DrawingSegment[]) => DrawingSegment[])
  ) => void;
  selectedDrawingId: string | null;
  selectedDrawingIds: string[];
  handleAddDrawingSegment: (params: CreateDrawingSegmentParams) => void;
  handleUpdateDrawingAnnotation: (
    id: string,
    updates: Partial<Annotation>
  ) => void;
  handleUpdateDrawingAnnotationLive: (
    id: string,
    updates: Partial<Annotation>
  ) => void;
  handleResizeDrawingSegment: (
    id: string,
    startTime: number,
    endTime: number
  ) => void;
  handleMoveDrawingSegment: (
    id: string,
    startTime: number,
    endTime: number
  ) => void;
  handleSplitDrawingSegment: (id: string, cutTime: number) => void;
  handleUpdateDrawingAnnotationsMultiple: (
    updates: Array<{ id: string; updates: Partial<Annotation> }>
  ) => void;
  handleCommitDrawingGesture: () => void;
  handleDeleteDrawingSegment: (id: string) => void;
  handleDeleteSelectedDrawings: () => void;
  handleSelectDrawingSegment: (
    id: string | null,
    addToSelection?: boolean
  ) => void;
  handleSelectMultipleDrawings: (ids: string[]) => void;
  clearDrawingSelection: () => void;
}

function getSegmentTimes(
  timelinePosition: number,
  totalTimelineDuration: number
): { startTime: number; endTime: number } {
  const duration = Math.min(
    DEFAULT_DRAWING_SEGMENT_DURATION,
    totalTimelineDuration
  );
  const maxStart = Math.max(0, totalTimelineDuration - duration);
  const startTime = Math.max(0, Math.min(timelinePosition, maxStart));
  const endTime = Math.min(startTime + duration, totalTimelineDuration);

  return { startTime, endTime };
}

export function useDrawingSegments({
  totalTimelineDuration,
  slice,
}: UseDrawingSegmentsProps): UseDrawingSegmentsReturn {
  const {
    value: drawingSegments,
    set: setDrawingSegments,
    setWithoutHistory,
    commit,
  } = slice;

  const [selectedDrawingIds, setSelectedDrawingIds] = useState<string[]>([]);
  const selectedDrawingId =
    selectedDrawingIds[selectedDrawingIds.length - 1] ?? null;
  const gestureActiveRef = useRef(false);
  const drawingSegmentsRef = useRef(drawingSegments);

  useEffect(() => {
    drawingSegmentsRef.current = drawingSegments;
  }, [drawingSegments]);

  useEffect(() => {
    setSelectedDrawingIds(prev => {
      if (prev.length === 0) return prev;
      const existingIds = new Set(drawingSegments.map(drawing => drawing.id));
      const next = prev.filter(id => existingIds.has(id));
      return next.length === prev.length ? prev : next;
    });
  }, [drawingSegments]);

  useEffect(() => {
    if (totalTimelineDuration === 0) return;

    const needsCleanup = drawingSegmentsRef.current.some(
      drawing =>
        drawing.startTime >= totalTimelineDuration ||
        drawing.endTime > totalTimelineDuration
    );

    if (!needsCleanup) return;

    setWithoutHistory(prev =>
      prev
        .filter(drawing => drawing.startTime < totalTimelineDuration)
        .map(drawing => ({
          ...drawing,
          endTime: Math.min(drawing.endTime, totalTimelineDuration),
        }))
        .filter(
          drawing =>
            drawing.endTime - drawing.startTime >= MIN_DRAWING_SEGMENT_DURATION
        )
    );
  }, [totalTimelineDuration, setWithoutHistory]);

  const handleAddDrawingSegment = useCallback(
    ({
      annotation,
      timelinePosition,
      canvasWidth,
      canvasHeight,
    }: CreateDrawingSegmentParams) => {
      if (totalTimelineDuration <= 0) return;

      const { startTime, endTime } = getSegmentTimes(
        timelinePosition,
        totalTimelineDuration
      );

      if (endTime - startTime < MIN_DRAWING_SEGMENT_DURATION) return;

      const newSegment: DrawingSegment = {
        id: crypto.randomUUID(),
        startTime,
        endTime,
        canvasWidth,
        canvasHeight,
        annotations: [{ ...annotation, id: crypto.randomUUID() }],
      };

      setDrawingSegments(prev => [...prev, newSegment]);
      setSelectedDrawingIds([newSegment.id]);
    },
    [totalTimelineDuration, setDrawingSegments]
  );

  const handleUpdateDrawingAnnotation = useCallback(
    (id: string, updates: Partial<Annotation>) => {
      setDrawingSegments(prev =>
        prev.map(drawing => {
          if (drawing.id !== id) return drawing;

          return {
            ...drawing,
            annotations: drawing.annotations.map(annotation => ({
              ...annotation,
              ...updates,
            })) as Annotation[],
          };
        })
      );
    },
    [setDrawingSegments]
  );

  const handleUpdateDrawingAnnotationLive = useCallback(
    (id: string, updates: Partial<Annotation>) => {
      gestureActiveRef.current = true;
      setWithoutHistory(prev =>
        prev.map(drawing => {
          if (drawing.id !== id) return drawing;

          return {
            ...drawing,
            annotations: drawing.annotations.map(annotation => ({
              ...annotation,
              ...updates,
            })) as Annotation[],
          };
        })
      );
    },
    [setWithoutHistory]
  );

  const handleUpdateDrawingAnnotationsMultiple = useCallback(
    (updates: Array<{ id: string; updates: Partial<Annotation> }>) => {
      if (updates.length === 0) return;
      gestureActiveRef.current = true;
      const updateMap = new Map(updates.map(item => [item.id, item.updates]));
      setWithoutHistory(prev =>
        prev.map(drawing => {
          const update = updateMap.get(drawing.id);
          if (!update) return drawing;

          return {
            ...drawing,
            annotations: drawing.annotations.map(annotation => ({
              ...annotation,
              ...update,
            })) as Annotation[],
          };
        })
      );
    },
    [setWithoutHistory]
  );

  const handleResizeDrawingSegment = useCallback(
    (id: string, startTime: number, endTime: number) => {
      gestureActiveRef.current = true;
      setWithoutHistory(prev =>
        prev.map(drawing => {
          if (drawing.id !== id) return drawing;

          const clampedStart = Math.max(
            0,
            Math.min(startTime, endTime - MIN_DRAWING_SEGMENT_DURATION)
          );
          const clampedEnd = Math.min(
            totalTimelineDuration,
            Math.max(endTime, clampedStart + MIN_DRAWING_SEGMENT_DURATION)
          );

          return {
            ...drawing,
            startTime: clampedStart,
            endTime: clampedEnd,
          };
        })
      );
    },
    [totalTimelineDuration, setWithoutHistory]
  );

  const handleMoveDrawingSegment = useCallback(
    (id: string, startTime: number, endTime: number) => {
      gestureActiveRef.current = true;
      setWithoutHistory(prev =>
        prev.map(drawing => {
          if (drawing.id !== id) return drawing;

          const duration = endTime - startTime;
          const maxStart = Math.max(0, totalTimelineDuration - duration);
          const clampedStart = Math.max(0, Math.min(startTime, maxStart));

          return {
            ...drawing,
            startTime: clampedStart,
            endTime: Math.min(clampedStart + duration, totalTimelineDuration),
          };
        })
      );
    },
    [totalTimelineDuration, setWithoutHistory]
  );

  const handleSplitDrawingSegment = useCallback(
    (id: string, cutTime: number) => {
      const drawing = drawingSegmentsRef.current.find(
        segment => segment.id === id
      );
      if (!drawing) return;

      const split = splitDrawingSegment(drawing, cutTime);
      if (!split) return;

      const [left, right] = split;
      setDrawingSegments(prev =>
        prev.flatMap(segment => (segment.id === id ? [left, right] : [segment]))
      );
    },
    [setDrawingSegments]
  );

  const handleCommitDrawingGesture = useCallback(() => {
    if (!gestureActiveRef.current) return;
    gestureActiveRef.current = false;
    commit();
  }, [commit]);

  const handleDeleteDrawingSegment = useCallback(
    (id: string) => {
      setDrawingSegments(prev => prev.filter(drawing => drawing.id !== id));
      setSelectedDrawingIds(prev =>
        prev.filter(selectedId => selectedId !== id)
      );
    },
    [setDrawingSegments]
  );

  const handleDeleteSelectedDrawings = useCallback(() => {
    setSelectedDrawingIds(prev => {
      if (prev.length === 0) return prev;
      const idsToDelete = new Set(prev);
      setDrawingSegments(segments =>
        segments.filter(drawing => !idsToDelete.has(drawing.id))
      );
      return [];
    });
  }, [setDrawingSegments]);

  const handleSelectDrawingSegment = useCallback(
    (id: string | null, addToSelection = false) => {
      if (id === null) {
        setSelectedDrawingIds([]);
        return;
      }

      if (!addToSelection) {
        setSelectedDrawingIds([id]);
        return;
      }

      setSelectedDrawingIds(prev =>
        prev.includes(id)
          ? prev.filter(selectedId => selectedId !== id)
          : [...prev, id]
      );
    },
    []
  );

  const handleSelectMultipleDrawings = useCallback((ids: string[]) => {
    setSelectedDrawingIds(ids);
  }, []);

  const clearDrawingSelection = useCallback(() => {
    setSelectedDrawingIds([]);
  }, []);

  return {
    drawingSegments,
    setDrawingSegments,
    selectedDrawingId,
    selectedDrawingIds,
    handleAddDrawingSegment,
    handleUpdateDrawingAnnotation,
    handleUpdateDrawingAnnotationLive,
    handleUpdateDrawingAnnotationsMultiple,
    handleResizeDrawingSegment,
    handleMoveDrawingSegment,
    handleSplitDrawingSegment,
    handleCommitDrawingGesture,
    handleDeleteDrawingSegment,
    handleDeleteSelectedDrawings,
    handleSelectDrawingSegment,
    handleSelectMultipleDrawings,
    clearDrawingSelection,
  };
}
