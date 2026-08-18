import { useState, useCallback, useMemo } from 'react';
import {
  DEFAULT_PIXELS_PER_SECOND,
  MIN_PIXELS_PER_SECOND,
  MAX_PIXELS_PER_SECOND,
  ZOOM_STEP,
} from './timeline-constants';

interface UseTimelineZoomOptions {
  initialPixelsPerSecond?: number;
  onZoomChange?: (pixelsPerSecond: number) => void;
}

interface UseTimelineZoomReturn {
  pixelsPerSecond: number;
  zoomIn: () => void;
  zoomOut: () => void;
  setZoomLevel: (pixels: number) => void;
  resetZoom: () => void;
  canZoomIn: boolean;
  canZoomOut: boolean;
}

export function useTimelineZoom(
  options: UseTimelineZoomOptions = {}
): UseTimelineZoomReturn {
  const { initialPixelsPerSecond = DEFAULT_PIXELS_PER_SECOND, onZoomChange } =
    options;

  const [pixelsPerSecond, setPixelsPerSecond] = useState(() =>
    Math.max(
      MIN_PIXELS_PER_SECOND,
      Math.min(MAX_PIXELS_PER_SECOND, initialPixelsPerSecond)
    )
  );
  const [previousInitialPixelsPerSecond, setPreviousInitialPixelsPerSecond] =
    useState(initialPixelsPerSecond);

  if (previousInitialPixelsPerSecond !== initialPixelsPerSecond) {
    setPreviousInitialPixelsPerSecond(initialPixelsPerSecond);
    if (
      initialPixelsPerSecond >= MIN_PIXELS_PER_SECOND &&
      initialPixelsPerSecond <= MAX_PIXELS_PER_SECOND
    ) {
      setPixelsPerSecond(initialPixelsPerSecond);
    }
  }

  const setZoomLevel = useCallback(
    (pixels: number) => {
      const clamped = Math.max(
        MIN_PIXELS_PER_SECOND,
        Math.min(MAX_PIXELS_PER_SECOND, pixels)
      );
      setPixelsPerSecond(clamped);
      onZoomChange?.(clamped);
    },
    [onZoomChange]
  );

  const zoomIn = useCallback(() => {
    setPixelsPerSecond(prev => {
      const next = Math.min(MAX_PIXELS_PER_SECOND, prev * ZOOM_STEP);
      onZoomChange?.(next);
      return next;
    });
  }, [onZoomChange]);

  const zoomOut = useCallback(() => {
    setPixelsPerSecond(prev => {
      const next = Math.max(MIN_PIXELS_PER_SECOND, prev / ZOOM_STEP);
      onZoomChange?.(next);
      return next;
    });
  }, [onZoomChange]);

  const resetZoom = useCallback(() => {
    setPixelsPerSecond(DEFAULT_PIXELS_PER_SECOND);
    onZoomChange?.(DEFAULT_PIXELS_PER_SECOND);
  }, [onZoomChange]);

  const canZoomIn = useMemo(
    () => pixelsPerSecond < MAX_PIXELS_PER_SECOND,
    [pixelsPerSecond]
  );

  const canZoomOut = useMemo(
    () => pixelsPerSecond > MIN_PIXELS_PER_SECOND,
    [pixelsPerSecond]
  );

  return {
    pixelsPerSecond,
    zoomIn,
    zoomOut,
    setZoomLevel,
    resetZoom,
    canZoomIn,
    canZoomOut,
  };
}
