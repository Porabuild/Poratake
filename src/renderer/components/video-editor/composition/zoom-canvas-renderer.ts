import {
  DEFAULT_ZOOM_SETTINGS,
  type ZoomSegment,
  type ZoomSettings,
} from '@/types/zoom';
import type { CursorData } from '@/types/cursor';
import type { VideoSegment } from '@/types/video';
import type { ViewportState } from './types';
import { clamp } from '@/types/geometry';
import {
  getZoomState,
  simulateViewport,
  calculateOptimalCenter,
  clearViewportKeyframeCache,
  type ZoomState,
  type ViewportTransform,
  type Position,
} from './zoom-logic';

export type { ZoomState, ViewportTransform };

export interface ZoomTransform extends ViewportTransform {
  viewport?: ViewportState;
}

export interface ZoomTransformOptions {
  fps: number;
}

const optimalCenterCache = new Map<string, Position>();

function getOptimalCenterCached(
  segmentId: string,
  cursorData: CursorData,
  videoSegments: VideoSegment[],
  segment: ZoomSegment,
  viewportSize: number,
  transitionInDuration: number
): Position {
  const cacheKey = `${segmentId}-${viewportSize.toFixed(4)}-${transitionInDuration.toFixed(2)}`;

  const cached = optimalCenterCache.get(cacheKey);
  if (cached) {
    return cached;
  }

  const center = calculateOptimalCenter(
    cursorData,
    videoSegments,
    segment,
    viewportSize,
    transitionInDuration
  );

  optimalCenterCache.set(cacheKey, center);
  return center;
}

export function clearOptimalCenterCache(): void {
  optimalCenterCache.clear();
  clearViewportKeyframeCache();
}

function resolveZoomSettings(
  zoomSettings: ZoomSettings | null | undefined
): ZoomSettings | null {
  if (!zoomSettings) return null;
  return { ...DEFAULT_ZOOM_SETTINGS, ...zoomSettings };
}

function getIdentityTransform(): ZoomTransform {
  return {
    scale: 1,
    translateX: 0,
    translateY: 0,
  };
}

function calculateManualFocusViewport(
  zoomState: ZoomState,
  viewportSize: number,
  maxViewport: number
): { viewportX: number; viewportY: number } {
  const segment = zoomState.segment!;
  const focusPoint = segment.focusPoint ?? { x: 0.5, y: 0.5 };
  const fullZoomViewportSize = 1 / segment.zoomLevel;

  const targetViewportX = clamp(
    focusPoint.x - fullZoomViewportSize / 2,
    0,
    1 - fullZoomViewportSize
  );
  const targetViewportY = clamp(
    focusPoint.y - fullZoomViewportSize / 2,
    0,
    1 - fullZoomViewportSize
  );

  if (zoomState.isTransitioningIn) {
    const progress = zoomState.transitionProgress;
    const easedProgress =
      progress < 0.5
        ? 2 * progress * progress
        : 1 - Math.pow(-2 * progress + 2, 2) / 2;

    const scaledViewportX =
      targetViewportX * (viewportSize / fullZoomViewportSize);
    const scaledViewportY =
      targetViewportY * (viewportSize / fullZoomViewportSize);

    const viewportX = clamp(scaledViewportX * easedProgress, 0, maxViewport);
    const viewportY = clamp(scaledViewportY * easedProgress, 0, maxViewport);
    return { viewportX, viewportY };
  }

  if (zoomState.isTransitioningOut) {
    const progress = zoomState.zoomOutProgress;
    const startCenterX = targetViewportX + fullZoomViewportSize / 2;
    const startCenterY = targetViewportY + fullZoomViewportSize / 2;
    const centerX = startCenterX + (0.5 - startCenterX) * progress;
    const centerY = startCenterY + (0.5 - startCenterY) * progress;

    const viewportX = clamp(centerX - viewportSize / 2, 0, maxViewport);
    const viewportY = clamp(centerY - viewportSize / 2, 0, maxViewport);
    return { viewportX, viewportY };
  }

  const viewportX = clamp(targetViewportX, 0, maxViewport);
  const viewportY = clamp(targetViewportY, 0, maxViewport);
  return { viewportX, viewportY };
}

function calculateCursorFollowViewport(
  zoomState: ZoomState,
  cursorData: CursorData | null | undefined,
  videoSegments: VideoSegment[],
  timelineTime: number,
  viewportSize: number,
  maxViewport: number,
  fps: number,
  zoomSettings: ZoomSettings
): { viewportX: number; viewportY: number } {
  const segment = zoomState.segment!;
  const fullZoomViewportSize = 1 / segment.zoomLevel;

  if (!cursorData || cursorData.events.length === 0) {
    const centeredFullViewportX = clamp(
      0.5 - fullZoomViewportSize / 2,
      0,
      1 - fullZoomViewportSize
    );
    const centeredFullViewportY = clamp(
      0.5 - fullZoomViewportSize / 2,
      0,
      1 - fullZoomViewportSize
    );

    if (zoomState.isTransitioningIn) {
      const progress = zoomState.transitionProgress;
      const easedProgress =
        progress < 0.5
          ? 2 * progress * progress
          : 1 - Math.pow(-2 * progress + 2, 2) / 2;

      const scaledViewportX =
        centeredFullViewportX * (viewportSize / fullZoomViewportSize);
      const scaledViewportY =
        centeredFullViewportY * (viewportSize / fullZoomViewportSize);

      const viewportX = clamp(scaledViewportX * easedProgress, 0, maxViewport);
      const viewportY = clamp(scaledViewportY * easedProgress, 0, maxViewport);
      return { viewportX, viewportY };
    }

    if (zoomState.isTransitioningOut) {
      const progress = zoomState.zoomOutProgress;
      const startCenterX = centeredFullViewportX + fullZoomViewportSize / 2;
      const startCenterY = centeredFullViewportY + fullZoomViewportSize / 2;
      const centerX = startCenterX + (0.5 - startCenterX) * progress;
      const centerY = startCenterY + (0.5 - startCenterY) * progress;

      const viewportX = clamp(centerX - viewportSize / 2, 0, maxViewport);
      const viewportY = clamp(centerY - viewportSize / 2, 0, maxViewport);
      return { viewportX, viewportY };
    }

    const viewportX = clamp(centeredFullViewportX, 0, maxViewport);
    const viewportY = clamp(centeredFullViewportY, 0, maxViewport);
    return { viewportX, viewportY };
  }

  const optimalCenter = getOptimalCenterCached(
    segment.id,
    cursorData,
    videoSegments,
    segment,
    fullZoomViewportSize,
    zoomState.effectiveTransitionIn
  );

  let queryTime: number;
  if (zoomState.isTransitioningOut) {
    queryTime = segment.endTime - zoomState.effectiveTransitionOut;
  } else {
    queryTime = timelineTime;
  }

  const viewport = simulateViewport({
    cursorData,
    videoSegments,
    segment,
    currentTime: queryTime,
    viewportSize: fullZoomViewportSize,
    fps,
    transitionInDuration: zoomState.effectiveTransitionIn,
    optimalCenter,
    followSmoothness: zoomSettings.followSmoothness,
    lookAhead: zoomSettings.lookAhead,
  });

  if (zoomState.isTransitioningIn) {
    const progress = zoomState.transitionProgress;
    const easedProgress =
      progress < 0.5
        ? 2 * progress * progress
        : 1 - Math.pow(-2 * progress + 2, 2) / 2;

    const scaledViewportX = viewport.x * (viewportSize / fullZoomViewportSize);
    const scaledViewportY = viewport.y * (viewportSize / fullZoomViewportSize);

    const viewportX = clamp(scaledViewportX * easedProgress, 0, maxViewport);
    const viewportY = clamp(scaledViewportY * easedProgress, 0, maxViewport);
    return { viewportX, viewportY };
  }

  if (zoomState.isTransitioningOut) {
    const progress = zoomState.zoomOutProgress;
    const startCenterX = viewport.x + fullZoomViewportSize / 2;
    const startCenterY = viewport.y + fullZoomViewportSize / 2;
    const centerX = startCenterX + (0.5 - startCenterX) * progress;
    const centerY = startCenterY + (0.5 - startCenterY) * progress;

    const viewportX = clamp(centerX - viewportSize / 2, 0, maxViewport);
    const viewportY = clamp(centerY - viewportSize / 2, 0, maxViewport);
    return { viewportX, viewportY };
  }

  const viewportX = clamp(viewport.x, 0, maxViewport);
  const viewportY = clamp(viewport.y, 0, maxViewport);
  return { viewportX, viewportY };
}

export function calculateZoomTransform(
  zoomSegments: ZoomSegment[] | null | undefined,
  zoomSettings: ZoomSettings | null | undefined,
  cursorData: CursorData | null | undefined,
  videoSegments: VideoSegment[],
  timelineTime: number,
  videoWidth: number,
  videoHeight: number,
  options: ZoomTransformOptions
): ZoomTransform {
  const effectiveZoomSettings = resolveZoomSettings(zoomSettings);

  if (!zoomSegments || !effectiveZoomSettings || zoomSegments.length === 0) {
    return getIdentityTransform();
  }

  const zoomState = getZoomState(
    timelineTime,
    zoomSegments,
    effectiveZoomSettings
  );

  if (!zoomState.isZooming || zoomState.scale === 1 || !zoomState.segment) {
    return getIdentityTransform();
  }

  const viewportSize = 1 / zoomState.scale;
  const maxViewport = 1 - viewportSize;

  const isManualMode = zoomState.segment.targetMode === 'manual';

  const { viewportX, viewportY } = isManualMode
    ? calculateManualFocusViewport(zoomState, viewportSize, maxViewport)
    : calculateCursorFollowViewport(
        zoomState,
        cursorData,
        videoSegments,
        timelineTime,
        viewportSize,
        maxViewport,
        options.fps,
        effectiveZoomSettings
      );

  const translateX = -viewportX * videoWidth * zoomState.scale;
  const translateY = -viewportY * videoHeight * zoomState.scale;

  return {
    scale: zoomState.scale,
    translateX,
    translateY,
    viewport: { x: viewportX, y: viewportY },
  };
}
