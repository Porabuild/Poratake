import { useRef, useCallback, useMemo } from 'react';
import { DEFAULT_PIXELS_PER_SECOND } from './timeline-constants';
import { TimelineContext } from './timeline-context-value';
import { useTimelineZoom } from './use-timeline-zoom';

interface TimelineProviderProps {
  children: React.ReactNode;
  initialPixelsPerSecond?: number;
  onZoomChange?: (pixelsPerSecond: number) => void;
}

export function TimelineProvider({
  children,
  initialPixelsPerSecond = DEFAULT_PIXELS_PER_SECOND,
  onZoomChange,
}: TimelineProviderProps) {
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const zoom = useTimelineZoom({
    initialPixelsPerSecond,
    onZoomChange,
  });

  const timeToPixels = useCallback(
    (time: number): number => time * zoom.pixelsPerSecond,
    [zoom.pixelsPerSecond]
  );

  const pixelsToTime = useCallback(
    (pixels: number): number => pixels / zoom.pixelsPerSecond,
    [zoom.pixelsPerSecond]
  );

  const value = useMemo(
    () => ({
      pixelsPerSecond: zoom.pixelsPerSecond,
      scrollContainerRef,
      timeToPixels,
      pixelsToTime,
      zoomIn: zoom.zoomIn,
      zoomOut: zoom.zoomOut,
      setZoomLevel: zoom.setZoomLevel,
      resetZoom: zoom.resetZoom,
      canZoomIn: zoom.canZoomIn,
      canZoomOut: zoom.canZoomOut,
    }),
    [zoom, timeToPixels, pixelsToTime]
  );

  return (
    <TimelineContext.Provider value={value}>
      {children}
    </TimelineContext.Provider>
  );
}
