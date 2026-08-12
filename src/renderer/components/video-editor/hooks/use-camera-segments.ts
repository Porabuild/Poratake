import { useState, useCallback, useRef, useEffect } from 'react';
import type { CameraSegment, CameraVisibleRange } from '@/types/camera';
import {
  MIN_CAMERA_SEGMENT_DURATION,
  mapVideoRangesToCameraSegments,
} from '@/types/camera';
import type { Segment } from '../types';
import { splitTrackSegments } from '../timeline-split';
import type { SliceController } from './use-editor-history';

interface UseCameraSegmentsProps {
  ready: boolean;
  hasCameraData: boolean;
  hasSavedSegments: boolean;
  initialVisibleRanges: CameraVisibleRange[] | null;
  segments: Segment[];
  totalTimelineDuration: number;
  slice: SliceController<CameraSegment[]>;
}

interface UseCameraSegmentsReturn {
  cameraSegments: CameraSegment[];
  selectedCameraId: string | null;
  handleSplitCamera: (cutTime: number) => void;
  handleUpdateCamera: (id: string, startTime: number, endTime: number) => void;
  handleCommitCameraGesture: () => void;
  handleAddCamera: (startTime: number, endTime: number) => void;
  handleDeleteCamera: (id: string) => void;
  handleCameraSelect: (id: string | null) => void;
  clearCameraSelection: () => void;
}

export function useCameraSegments({
  ready,
  hasCameraData,
  hasSavedSegments,
  initialVisibleRanges,
  segments,
  totalTimelineDuration,
  slice,
}: UseCameraSegmentsProps): UseCameraSegmentsReturn {
  const {
    value: cameraSegments,
    set: setCameraSegments,
    setWithoutHistory,
    commit,
  } = slice;

  const [selectedCameraId, setSelectedCameraId] = useState<string | null>(null);
  const selectedCameraIdRef = useRef<string | null>(null);
  const gestureActiveRef = useRef(false);
  const initializedRef = useRef(false);

  const cameraSegmentsRef = useRef(cameraSegments);
  useEffect(() => {
    cameraSegmentsRef.current = cameraSegments;
  }, [cameraSegments]);

  useEffect(() => {
    if (!ready || !hasCameraData || initializedRef.current) return;
    initializedRef.current = true;

    if (hasSavedSegments) return;

    setWithoutHistory(
      mapVideoRangesToCameraSegments(
        initialVisibleRanges,
        segments,
        totalTimelineDuration
      )
    );
  }, [
    ready,
    hasCameraData,
    hasSavedSegments,
    initialVisibleRanges,
    segments,
    totalTimelineDuration,
    setWithoutHistory,
  ]);

  useEffect(() => {
    if (totalTimelineDuration === 0) return;

    const needsCleanup = cameraSegmentsRef.current.some(
      segment =>
        segment.startTime >= totalTimelineDuration ||
        segment.endTime > totalTimelineDuration
    );

    if (!needsCleanup) return;

    setWithoutHistory(prev =>
      prev
        .filter(segment => segment.startTime < totalTimelineDuration)
        .map(segment => ({
          ...segment,
          endTime: Math.min(segment.endTime, totalTimelineDuration),
        }))
        .filter(
          segment =>
            segment.endTime - segment.startTime >= MIN_CAMERA_SEGMENT_DURATION
        )
    );
  }, [totalTimelineDuration, setWithoutHistory]);

  const wouldOverlap = useCallback(
    (startTime: number, endTime: number, excludeId?: string): boolean => {
      return cameraSegmentsRef.current.some(segment => {
        if (segment.id === excludeId) return false;
        return startTime < segment.endTime && endTime > segment.startTime;
      });
    },
    []
  );

  const handleSplitCamera = useCallback(
    (cutTime: number) => {
      const next = splitTrackSegments(cameraSegmentsRef.current, cutTime);
      if (next === cameraSegmentsRef.current) return;
      setCameraSegments(next);
    },
    [setCameraSegments]
  );

  const handleUpdateCamera = useCallback(
    (id: string, startTime: number, endTime: number) => {
      if (wouldOverlap(startTime, endTime, id)) return;
      gestureActiveRef.current = true;
      setWithoutHistory(prev =>
        prev.map(segment =>
          segment.id === id ? { ...segment, startTime, endTime } : segment
        )
      );
    },
    [wouldOverlap, setWithoutHistory]
  );

  const handleCommitCameraGesture = useCallback(() => {
    if (!gestureActiveRef.current) return;
    gestureActiveRef.current = false;
    commit();
  }, [commit]);

  const handleAddCamera = useCallback(
    (startTime: number, endTime: number) => {
      if (wouldOverlap(startTime, endTime)) return;

      const newSegment: CameraSegment = {
        id: crypto.randomUUID(),
        startTime,
        endTime,
      };
      setCameraSegments(prev =>
        [...prev, newSegment].sort((a, b) => a.startTime - b.startTime)
      );
      setSelectedCameraId(newSegment.id);
      selectedCameraIdRef.current = newSegment.id;
    },
    [wouldOverlap, setCameraSegments]
  );

  const handleDeleteCamera = useCallback(
    (id: string) => {
      setCameraSegments(prev => prev.filter(segment => segment.id !== id));
      setSelectedCameraId(null);
      selectedCameraIdRef.current = null;
    },
    [setCameraSegments]
  );

  const handleCameraSelect = useCallback((id: string | null) => {
    setSelectedCameraId(id);
    selectedCameraIdRef.current = id;
  }, []);

  const clearCameraSelection = useCallback(() => {
    setSelectedCameraId(null);
    selectedCameraIdRef.current = null;
  }, []);

  return {
    cameraSegments,
    selectedCameraId,
    handleSplitCamera,
    handleUpdateCamera,
    handleCommitCameraGesture,
    handleAddCamera,
    handleDeleteCamera,
    handleCameraSelect,
    clearCameraSelection,
  };
}
