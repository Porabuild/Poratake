import type { CameraStyle } from '@/types/camera';
import type { CursorData, CursorEvent } from '@/types/cursor';
import type { VideoSegment } from '@/types/video';
import {
  type Context2D,
  type ViewportState,
  mapTimelineToVideoTime,
} from './types';
import {
  DEFAULT_CAMERA_STYLE,
  getCameraPositionCoords,
  getCameraOverlayDimensions,
} from '@/types/camera';
import { calculateShadowConfig } from './wallpaper-canvas-renderer';

export interface ZoomInfo {
  scale: number;
  viewport?: ViewportState;
}

const CURSOR_DODGE_CONFIG = {
  boundsPadding: 0.05,
  lookAheadTimeMs: 150,
  fadeOutDurationMs: 100,
  fadeInDurationMs: 200,
};

const CAMERA_SHADOW_COLOR = 'rgba(0, 0, 0, 1)';

export interface CameraRenderConfig {
  cameraStyle: CameraStyle;
  cursorData?: CursorData | null;
  segments: VideoSegment[];
  videoWidth: number;
  videoHeight: number;
  offsetX: number;
  offsetY: number;
  zoomInfo?: ZoomInfo;
}

interface CameraLayout {
  left: number;
  top: number;
  width: number;
  height: number;
  borderRadius: number;
  transformOriginX: number;
  transformOriginY: number;
}

const CAMERA_ZOOM_SHRINK_FACTOR = 0.35;
const MIN_CAMERA_SCALE = 0.5;

function getCameraZoomScale(zoomInfo: ZoomInfo | undefined): number {
  if (!zoomInfo || zoomInfo.scale <= 1) return 1;
  const shrink = (zoomInfo.scale - 1) * CAMERA_ZOOM_SHRINK_FACTOR;
  return Math.max(MIN_CAMERA_SCALE, 1 - shrink);
}

function calculateCameraLayout(
  cameraStyle: CameraStyle,
  videoWidth: number,
  videoHeight: number,
  zoomInfo?: ZoomInfo
): CameraLayout {
  const { width: baseWidth, height: baseHeight } = getCameraOverlayDimensions(
    videoWidth,
    videoHeight,
    cameraStyle.size,
    cameraStyle.shape
  );

  const cameraScale = getCameraZoomScale(zoomInfo);
  const width = baseWidth * cameraScale;
  const height = baseHeight * cameraScale;

  const coords = getCameraPositionCoords(cameraStyle.position, 0);
  const minDimension = Math.min(videoWidth, videoHeight);
  const paddingPx = (cameraStyle.padding / 100) * minDimension;

  let left: number;
  let top: number;
  let transformOriginX: number;
  let transformOriginY: number;

  switch (coords.anchorX) {
    case 'left':
      left = paddingPx;
      transformOriginX = 0;
      break;
    case 'center':
      left = videoWidth / 2 - width / 2;
      transformOriginX = 0.5;
      break;
    case 'right':
      left = videoWidth - width - paddingPx;
      transformOriginX = 1;
      break;
  }

  switch (coords.anchorY) {
    case 'top':
      top = paddingPx;
      transformOriginY = 0;
      break;
    case 'center':
      top = videoHeight / 2 - height / 2;
      transformOriginY = 0.5;
      break;
    case 'bottom':
      top = videoHeight - height - paddingPx;
      transformOriginY = 1;
      break;
  }

  const minOverlayDimension = Math.min(width, height);
  const maxBorderRadius = minOverlayDimension / 2;
  const borderRadius = (cameraStyle.borderRadius / 100) * maxBorderRadius;

  return {
    left,
    top,
    width,
    height,
    borderRadius,
    transformOriginX,
    transformOriginY,
  };
}

function binarySearchEvents(events: CursorEvent[], timestamp: number): number {
  let left = 0;
  let right = events.length - 1;
  let result = -1;

  while (left <= right) {
    const mid = Math.floor((left + right) / 2);
    if (events[mid].timestamp <= timestamp) {
      result = mid;
      left = mid + 1;
    } else {
      right = mid - 1;
    }
  }

  return result;
}

function getCursorPositionAtTime(
  events: CursorEvent[],
  timestamp: number
): { x: number; y: number } | null {
  if (events.length === 0) return null;

  const beforeIdx = binarySearchEvents(events, timestamp);
  if (beforeIdx === -1) return { x: events[0].x, y: events[0].y };
  if (beforeIdx === events.length - 1) {
    return { x: events[beforeIdx].x, y: events[beforeIdx].y };
  }

  const before = events[beforeIdx];
  const after = events[beforeIdx + 1];
  const t =
    (timestamp - before.timestamp) / (after.timestamp - before.timestamp);

  return {
    x: before.x + (after.x - before.x) * t,
    y: before.y + (after.y - before.y) * t,
  };
}

function transformCursorToScreenSpace(
  cursorX: number,
  cursorY: number,
  zoomInfo: ZoomInfo | undefined
): { x: number; y: number } {
  if (!zoomInfo || zoomInfo.scale === 1 || !zoomInfo.viewport) {
    return { x: cursorX, y: cursorY };
  }

  const viewportSize = 1 / zoomInfo.scale;
  const viewportX = zoomInfo.viewport.x;
  const viewportY = zoomInfo.viewport.y;

  const screenX = (cursorX - viewportX) / viewportSize;
  const screenY = (cursorY - viewportY) / viewportSize;

  return { x: screenX, y: screenY };
}

function isCursorInCameraBounds(
  cursorX: number,
  cursorY: number,
  cameraLeft: number,
  cameraTop: number,
  cameraWidth: number,
  cameraHeight: number,
  videoWidth: number,
  videoHeight: number,
  padding: number
): boolean {
  const normalizedLeft = cameraLeft / videoWidth;
  const normalizedTop = cameraTop / videoHeight;
  const normalizedRight = (cameraLeft + cameraWidth) / videoWidth;
  const normalizedBottom = (cameraTop + cameraHeight) / videoHeight;

  const paddedLeft = normalizedLeft - padding;
  const paddedTop = normalizedTop - padding;
  const paddedRight = normalizedRight + padding;
  const paddedBottom = normalizedBottom + padding;

  return (
    cursorX >= paddedLeft &&
    cursorX <= paddedRight &&
    cursorY >= paddedTop &&
    cursorY <= paddedBottom
  );
}

function easeOutCubic(t: number): number {
  return 1 - Math.pow(1 - t, 3);
}

function easeInCubic(t: number): number {
  return t * t * t;
}

function findLastExitTime(
  events: CursorEvent[],
  videoTime: number,
  layout: CameraLayout,
  videoWidth: number,
  videoHeight: number,
  searchWindowMs: number,
  zoomInfo: ZoomInfo | undefined
): number | null {
  const searchStartTime = Math.max(0, videoTime - searchWindowMs / 1000);
  const stepMs = 10;

  let lastExitTime: number | null = null;
  let wasInBounds = false;

  for (let t = searchStartTime; t <= videoTime; t += stepMs / 1000) {
    const pos = getCursorPositionAtTime(events, t);
    if (!pos) continue;

    const screenPos = transformCursorToScreenSpace(pos.x, pos.y, zoomInfo);

    const isInBounds = isCursorInCameraBounds(
      screenPos.x,
      screenPos.y,
      layout.left,
      layout.top,
      layout.width,
      layout.height,
      videoWidth,
      videoHeight,
      CURSOR_DODGE_CONFIG.boundsPadding
    );

    if (!isInBounds && wasInBounds) {
      lastExitTime = t;
    }
    wasInBounds = isInBounds;
  }

  return lastExitTime;
}

function findTimeToEntry(
  events: CursorEvent[],
  videoTime: number,
  layout: CameraLayout,
  videoWidth: number,
  videoHeight: number,
  maxLookAheadMs: number,
  zoomInfo: ZoomInfo | undefined
): number | null {
  const stepMs = 5;

  for (let offset = 0; offset <= maxLookAheadMs; offset += stepMs) {
    const t = videoTime + offset / 1000;
    const pos = getCursorPositionAtTime(events, t);
    if (!pos) continue;

    const screenPos = transformCursorToScreenSpace(pos.x, pos.y, zoomInfo);

    const isInBounds = isCursorInCameraBounds(
      screenPos.x,
      screenPos.y,
      layout.left,
      layout.top,
      layout.width,
      layout.height,
      videoWidth,
      videoHeight,
      CURSOR_DODGE_CONFIG.boundsPadding
    );

    if (isInBounds) {
      return offset;
    }
  }

  return null;
}

function calculateCameraOpacity(
  cursorData: CursorData | null | undefined,
  segments: VideoSegment[],
  timelineTime: number,
  layout: CameraLayout,
  videoWidth: number,
  videoHeight: number,
  zoomInfo: ZoomInfo | undefined
): number {
  if (!cursorData?.events?.length) return 1;

  const videoTime = mapTimelineToVideoTime(timelineTime, segments);
  if (videoTime === null) return 1;

  const cursorPos = getCursorPositionAtTime(cursorData.events, videoTime);
  if (!cursorPos) return 1;

  const screenPos = transformCursorToScreenSpace(
    cursorPos.x,
    cursorPos.y,
    zoomInfo
  );

  const isCurrentlyInBounds = isCursorInCameraBounds(
    screenPos.x,
    screenPos.y,
    layout.left,
    layout.top,
    layout.width,
    layout.height,
    videoWidth,
    videoHeight,
    CURSOR_DODGE_CONFIG.boundsPadding
  );

  if (isCurrentlyInBounds) {
    return 0;
  }

  const timeToEntry = findTimeToEntry(
    cursorData.events,
    videoTime,
    layout,
    videoWidth,
    videoHeight,
    CURSOR_DODGE_CONFIG.lookAheadTimeMs,
    zoomInfo
  );

  if (timeToEntry !== null) {
    const progress = 1 - timeToEntry / CURSOR_DODGE_CONFIG.lookAheadTimeMs;
    return 1 - easeInCubic(Math.min(1, progress * 2));
  }

  const lastExitTime = findLastExitTime(
    cursorData.events,
    videoTime,
    layout,
    videoWidth,
    videoHeight,
    CURSOR_DODGE_CONFIG.fadeInDurationMs,
    zoomInfo
  );

  if (lastExitTime !== null) {
    const timeSinceExit = (videoTime - lastExitTime) * 1000;
    if (timeSinceExit < CURSOR_DODGE_CONFIG.fadeInDurationMs) {
      const progress = timeSinceExit / CURSOR_DODGE_CONFIG.fadeInDurationMs;
      return easeOutCubic(progress);
    }
  }

  return 1;
}

function renderShadow(
  ctx: Context2D,
  x: number,
  y: number,
  width: number,
  height: number,
  borderRadius: number,
  shadow: number
): void {
  if (shadow <= 0) return;

  const shadowConfig = calculateShadowConfig(shadow);
  if (!shadowConfig) return;

  ctx.save();

  const shadowSpread = shadowConfig.blur;
  const expandedX = x - shadowSpread;
  const expandedY = y - shadowSpread;
  const expandedWidth = width + shadowSpread * 2;
  const expandedHeight = height + shadowSpread * 2;

  ctx.beginPath();
  ctx.rect(expandedX, expandedY, expandedWidth, expandedHeight);
  if (borderRadius > 0) {
    ctx.roundRect(x, y, width, height, borderRadius);
  } else {
    ctx.rect(x, y, width, height);
  }
  ctx.clip('evenodd');

  ctx.shadowColor = `rgba(0, 0, 0, ${shadowConfig.opacity})`;
  ctx.shadowBlur = shadowConfig.blur;
  ctx.shadowOffsetX = 0;
  ctx.shadowOffsetY = shadowConfig.offsetY;

  ctx.beginPath();
  if (borderRadius > 0) {
    ctx.roundRect(x, y, width, height, borderRadius);
  } else {
    ctx.rect(x, y, width, height);
  }
  ctx.fillStyle = CAMERA_SHADOW_COLOR;
  ctx.fill();

  ctx.restore();
}

export function renderCamera(
  ctx: Context2D,
  timelineTime: number,
  cameraSource:
    | HTMLVideoElement
    | VideoFrame
    | ImageBitmap
    | HTMLCanvasElement
    | OffscreenCanvas,
  config: CameraRenderConfig
): void {
  const {
    cameraStyle,
    cursorData,
    segments,
    videoWidth,
    videoHeight,
    zoomInfo,
  } = config;
  const effectiveStyle = cameraStyle ?? DEFAULT_CAMERA_STYLE;

  if (!effectiveStyle.visible) return;

  const layout = calculateCameraLayout(
    effectiveStyle,
    videoWidth,
    videoHeight,
    zoomInfo
  );
  const opacity = calculateCameraOpacity(
    cursorData,
    segments,
    timelineTime,
    layout,
    videoWidth,
    videoHeight,
    zoomInfo
  );

  if (opacity <= 0) return;

  const finalLeft = config.offsetX + layout.left;
  const finalTop = config.offsetY + layout.top;

  ctx.save();
  ctx.globalAlpha = opacity;

  renderShadow(
    ctx,
    finalLeft,
    finalTop,
    layout.width,
    layout.height,
    layout.borderRadius,
    effectiveStyle.shadow
  );

  ctx.beginPath();
  if (layout.borderRadius > 0) {
    ctx.roundRect(
      finalLeft,
      finalTop,
      layout.width,
      layout.height,
      layout.borderRadius
    );
  } else {
    ctx.rect(finalLeft, finalTop, layout.width, layout.height);
  }
  ctx.clip();

  const centerX = finalLeft + layout.width / 2;
  const centerY = finalTop + layout.height / 2;

  if (effectiveStyle.mirrored) {
    ctx.translate(centerX, centerY);
    ctx.scale(-1, 1);
    ctx.translate(-centerX, -centerY);
  }

  let sourceWidth: number;
  let sourceHeight: number;

  if (cameraSource instanceof HTMLVideoElement) {
    sourceWidth = cameraSource.videoWidth;
    sourceHeight = cameraSource.videoHeight;
  } else if ('displayWidth' in cameraSource) {
    sourceWidth = cameraSource.displayWidth;
    sourceHeight = cameraSource.displayHeight;
  } else {
    sourceWidth = cameraSource.width;
    sourceHeight = cameraSource.height;
  }

  if (sourceWidth === 0 || sourceHeight === 0) {
    ctx.restore();
    return;
  }

  const sourceAspect = sourceWidth / sourceHeight;
  const targetAspect = layout.width / layout.height;

  let drawWidth: number;
  let drawHeight: number;
  let drawX: number;
  let drawY: number;

  if (sourceAspect > targetAspect) {
    drawHeight = layout.height;
    drawWidth = layout.height * sourceAspect;
    drawX = finalLeft - (drawWidth - layout.width) / 2;
    drawY = finalTop;
  } else {
    drawWidth = layout.width;
    drawHeight = layout.width / sourceAspect;
    drawX = finalLeft;
    drawY = finalTop - (drawHeight - layout.height) / 2;
  }

  ctx.drawImage(cameraSource, drawX, drawY, drawWidth, drawHeight);

  ctx.restore();
}

export function getCameraLayoutInfo(
  cameraStyle: CameraStyle | null,
  videoWidth: number,
  videoHeight: number
): CameraLayout | null {
  if (!cameraStyle?.visible) return null;
  return calculateCameraLayout(cameraStyle, videoWidth, videoHeight);
}
