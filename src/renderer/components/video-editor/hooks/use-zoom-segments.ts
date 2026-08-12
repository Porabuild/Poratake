import { useState, useCallback, useRef, useEffect } from 'react';
import type { ZoomSegment, ZoomSettings } from '@/types/zoom';
import { DEFAULT_ZOOM_LEVEL } from '@/types/zoom';
import type { SidebarTab } from '../editor-sidebar';
import { splitTrackSegments } from '../timeline-split';
import type { SliceController } from './use-editor-history';

interface UseZoomSegmentsProps {
  totalTimelineDuration: number;
  activateSidebarTab: (tab: SidebarTab) => void;
  segmentsSlice: SliceController<ZoomSegment[]>;
  settingsSlice: SliceController<ZoomSettings>;
}

interface UseZoomSegmentsReturn {
  zoomSegments: ZoomSegment[];
  setZoomSegments: (
    updater: ZoomSegment[] | ((prev: ZoomSegment[]) => ZoomSegment[])
  ) => void;
  selectedZoomId: string | null;
  zoomSettings: ZoomSettings;
  setZoomSettings: (
    updater: ZoomSettings | ((prev: ZoomSettings) => ZoomSettings)
  ) => void;
  handleAddZoom: (startTime: number, endTime: number) => void;
  handleUpdateZoom: (id: string, startTime: number, endTime: number) => void;
  handleSplitZoom: (cutTime: number) => void;
  handleCommitZoomGesture: () => void;
  handleDeleteZoom: (id: string) => void;
  handleApplyZoomToAll: (id: string) => void;
  handleDeleteOtherZooms: (id: string) => void;
  handleUpdateZoomLevel: (id: string, zoomLevel: number) => void;
  handleUpdateZoomSegment: (id: string, updates: Partial<ZoomSegment>) => void;
  handleZoomSelect: (id: string | null) => void;
  clearZoomSelection: () => void;
}

export function useZoomSegments({
  totalTimelineDuration,
  activateSidebarTab,
  segmentsSlice,
  settingsSlice,
}: UseZoomSegmentsProps): UseZoomSegmentsReturn {
  const {
    value: zoomSegments,
    set: setZoomSegments,
    setWithoutHistory,
    commit,
  } = segmentsSlice;
  const { value: zoomSettings, set: setZoomSettings } = settingsSlice;

  const [selectedZoomId, setSelectedZoomId] = useState<string | null>(null);
  const selectedZoomIdRef = useRef<string | null>(null);
  const gestureActiveRef = useRef(false);

  const zoomSegmentsRef = useRef(zoomSegments);
  useEffect(() => {
    zoomSegmentsRef.current = zoomSegments;
  }, [zoomSegments]);

  useEffect(() => {
    if (totalTimelineDuration === 0) return;

    const needsCleanup = zoomSegmentsRef.current.some(
      zoom =>
        zoom.startTime >= totalTimelineDuration ||
        zoom.endTime > totalTimelineDuration
    );

    if (!needsCleanup) return;

    setWithoutHistory(prev =>
      prev
        .filter(zoom => zoom.startTime < totalTimelineDuration)
        .map(zoom => ({
          ...zoom,
          endTime: Math.min(zoom.endTime, totalTimelineDuration),
        }))
        .filter(zoom => zoom.endTime - zoom.startTime >= 0.1)
    );
  }, [totalTimelineDuration, setWithoutHistory]);

  const wouldOverlap = useCallback(
    (startTime: number, endTime: number, excludeId?: string): boolean => {
      return zoomSegmentsRef.current.some(seg => {
        if (seg.id === excludeId) return false;
        return startTime < seg.endTime && endTime > seg.startTime;
      });
    },
    []
  );

  const handleAddZoom = useCallback(
    (startTime: number, endTime: number) => {
      if (wouldOverlap(startTime, endTime)) return;

      const newSegment: ZoomSegment = {
        id: crypto.randomUUID(),
        startTime,
        endTime,
        zoomLevel: DEFAULT_ZOOM_LEVEL,
      };
      setZoomSegments(prev => [...prev, newSegment]);
      setSelectedZoomId(newSegment.id);
      selectedZoomIdRef.current = newSegment.id;
      activateSidebarTab('zoom');
    },
    [wouldOverlap, activateSidebarTab, setZoomSegments]
  );

  const handleUpdateZoom = useCallback(
    (id: string, startTime: number, endTime: number) => {
      if (wouldOverlap(startTime, endTime, id)) return;
      gestureActiveRef.current = true;
      setWithoutHistory(prev =>
        prev.map(seg => (seg.id === id ? { ...seg, startTime, endTime } : seg))
      );
    },
    [wouldOverlap, setWithoutHistory]
  );

  const handleSplitZoom = useCallback(
    (cutTime: number) => {
      const next = splitTrackSegments(zoomSegmentsRef.current, cutTime);
      if (next === zoomSegmentsRef.current) return;
      setZoomSegments(next);
    },
    [setZoomSegments]
  );

  const handleCommitZoomGesture = useCallback(() => {
    if (!gestureActiveRef.current) return;
    gestureActiveRef.current = false;
    commit();
  }, [commit]);

  const handleDeleteZoom = useCallback(
    (id: string) => {
      setZoomSegments(prev => prev.filter(seg => seg.id !== id));
      setSelectedZoomId(null);
      selectedZoomIdRef.current = null;
    },
    [setZoomSegments]
  );

  const handleApplyZoomToAll = useCallback(
    (id: string) => {
      const sourceSegment = zoomSegmentsRef.current.find(seg => seg.id === id);
      if (!sourceSegment) return;

      setZoomSegments(prev =>
        prev.map(seg => ({
          ...seg,
          zoomLevel: sourceSegment.zoomLevel,
          transitionInDuration: sourceSegment.transitionInDuration,
          transitionOutDuration: sourceSegment.transitionOutDuration,
        }))
      );
    },
    [setZoomSegments]
  );

  const handleDeleteOtherZooms = useCallback(
    (id: string) => {
      setZoomSegments(prev => prev.filter(seg => seg.id === id));
      setSelectedZoomId(id);
      selectedZoomIdRef.current = id;
    },
    [setZoomSegments]
  );

  const handleUpdateZoomLevel = useCallback(
    (id: string, zoomLevel: number) => {
      setZoomSegments(prev =>
        prev.map(seg => (seg.id === id ? { ...seg, zoomLevel } : seg))
      );
    },
    [setZoomSegments]
  );

  const handleUpdateZoomSegment = useCallback(
    (id: string, updates: Partial<ZoomSegment>) => {
      setZoomSegments(prev =>
        prev.map(seg => (seg.id === id ? { ...seg, ...updates } : seg))
      );
    },
    [setZoomSegments]
  );

  const handleZoomSelect = useCallback(
    (id: string | null) => {
      setSelectedZoomId(id);
      selectedZoomIdRef.current = id;
      if (id === null) return;
      activateSidebarTab('zoom');
    },
    [activateSidebarTab]
  );

  const clearZoomSelection = useCallback(() => {
    setSelectedZoomId(null);
    selectedZoomIdRef.current = null;
  }, []);

  return {
    zoomSegments,
    setZoomSegments,
    selectedZoomId,
    zoomSettings,
    setZoomSettings,
    handleAddZoom,
    handleUpdateZoom,
    handleSplitZoom,
    handleCommitZoomGesture,
    handleDeleteZoom,
    handleApplyZoomToAll,
    handleDeleteOtherZooms,
    handleUpdateZoomLevel,
    handleUpdateZoomSegment,
    handleZoomSelect,
    clearZoomSelection,
  };
}
