import type {
  CursorType,
  CursorStyle,
  CursorData,
  CursorEvent,
} from '@/types/cursor';
import type { VideoSegment } from '@/types/video';
import type { Context2D } from './types';
import { mapTimelineToVideoTime } from './types';
import {
  calculateClickBounceScale,
  calculateIdleOpacity,
  interpolateCursorPosition,
} from './cursor-logic';
import { generateCursorSvg, getCursorHotspot } from './cursor-svg-data';

interface CursorRenderConfig {
  cursorData: CursorData;
  cursorStyle: CursorStyle;
  segments: VideoSegment[];
  videoWidth: number;
  videoHeight: number;
  offsetX: number;
  offsetY: number;
}

interface CachedCursor {
  image: HTMLImageElement;
  fill: string;
  stroke: string;
}

interface CachedCustomCursor {
  image: HTMLImageElement;
  dataUrl: string;
}

const cursorImageCache = new Map<string, CachedCursor>();
const customCursorCache = new Map<string, CachedCustomCursor>();

function getCacheKey(type: CursorType, fill: string, stroke: string): string {
  return `${type}:${fill}:${stroke}`;
}

function loadCustomCursorImage(dataUrl: string): HTMLImageElement | null {
  const cached = customCursorCache.get(dataUrl);
  if (cached && cached.dataUrl === dataUrl) {
    return cached.image;
  }

  const image = new Image();
  image.src = dataUrl;

  image.onload = () => {
    customCursorCache.set(dataUrl, { image, dataUrl });
  };

  if (image.complete && image.naturalWidth > 0) {
    customCursorCache.set(dataUrl, { image, dataUrl });
    return image;
  }

  return null;
}

function loadCursorImage(
  type: CursorType,
  fill: string,
  stroke: string
): HTMLImageElement | null {
  const key = getCacheKey(type, fill, stroke);
  const cached = cursorImageCache.get(key);

  if (cached && cached.fill === fill && cached.stroke === stroke) {
    return cached.image;
  }

  const svg = generateCursorSvg(type, fill, stroke);
  const blob = new Blob([svg], { type: 'image/svg+xml' });
  const url = URL.createObjectURL(blob);

  const image = new Image();
  image.src = url;

  image.onload = () => {
    URL.revokeObjectURL(url);
    cursorImageCache.set(key, { image, fill, stroke });
  };

  image.onerror = () => {
    URL.revokeObjectURL(url);
  };

  if (image.complete && image.naturalWidth > 0) {
    cursorImageCache.set(key, { image, fill, stroke });
    return image;
  }

  return null;
}

export function preloadCursorImages(
  types: CursorType[],
  fill: string,
  stroke: string
): void {
  types.forEach(type => loadCursorImage(type, fill, stroke));
}

export function preloadCustomCursorImage(dataUrl: string): void {
  loadCustomCursorImage(dataUrl);
}

const LIFE_SIZE_SPRITE_PX = 49;
const MAX_DISPLAY_SCALE = 2.5;

export function resolveCursorSpriteSize(
  sizePercent: number,
  videoHeight: number,
  recordingHeight: number
): number {
  const scale =
    Number.isFinite(recordingHeight) && recordingHeight > 0
      ? videoHeight / recordingHeight
      : 1;
  const displayScale = Math.min(Math.max(scale, 1), MAX_DISPLAY_SCALE);
  return (sizePercent / 100) * LIFE_SIZE_SPRITE_PX * displayScale;
}

const MOTION_BLUR_SAMPLES = 9;
const MOTION_BLUR_SHUTTER = 0.04;
const MOTION_BLUR_MIN_TRAVEL = 1.5;

let motionBlurBuffer: OffscreenCanvas | null = null;
let motionBlurBufferCtx: OffscreenCanvasRenderingContext2D | null = null;

function getMotionBlurBuffer(
  width: number,
  height: number
): { canvas: OffscreenCanvas; ctx: OffscreenCanvasRenderingContext2D } | null {
  if (typeof OffscreenCanvas === 'undefined') return null;

  if (
    !motionBlurBuffer ||
    motionBlurBuffer.width < width ||
    motionBlurBuffer.height < height
  ) {
    motionBlurBuffer = new OffscreenCanvas(width, height);
    motionBlurBufferCtx = motionBlurBuffer.getContext('2d');
  }

  if (!motionBlurBufferCtx) return null;
  return { canvas: motionBlurBuffer, ctx: motionBlurBufferCtx };
}

function getMotionBlurOffset(
  events: CursorEvent[],
  videoTime: number,
  smoothing: number,
  videoWidth: number,
  videoHeight: number,
  strength: number
): { dx: number; dy: number } {
  const half = (MOTION_BLUR_SHUTTER * strength) / 2;

  const start = interpolateCursorPosition(
    events,
    Math.max(0, videoTime - half),
    smoothing
  );
  const end = interpolateCursorPosition(events, videoTime + half, smoothing);
  if (!start || !end) return { dx: 0, dy: 0 };

  return {
    dx: (end.x - start.x) * videoWidth,
    dy: (end.y - start.y) * videoHeight,
  };
}

export function renderCursor(
  ctx: Context2D,
  timelineTime: number,
  config: CursorRenderConfig
): void {
  const { cursorData, cursorStyle, segments, videoWidth, videoHeight } = config;

  const videoTime = mapTimelineToVideoTime(timelineTime, segments);
  if (videoTime === null) return;

  const cursorState = interpolateCursorPosition(
    cursorData.events,
    videoTime,
    cursorStyle.smoothing
  );
  if (!cursorState) return;

  const x = cursorState.x * videoWidth + config.offsetX;
  const y = cursorState.y * videoHeight + config.offsetY;

  let opacity = 1;
  if (cursorStyle.hideOnIdle) {
    opacity = calculateIdleOpacity(
      cursorData.events,
      videoTime,
      cursorStyle.hideOnIdleTimeout
    );
    if (opacity <= 0) return;
  }

  const clickScale = cursorStyle.showClickHighlight
    ? calculateClickBounceScale(cursorState.clickProgress)
    : 1;

  const size = resolveCursorSpriteSize(
    cursorStyle.size,
    videoHeight,
    cursorData.recordingArea.height
  );

  const hasCustomCursor = !!cursorStyle.customCursorImage;
  const image = hasCustomCursor
    ? loadCustomCursorImage(cursorStyle.customCursorImage!)
    : loadCursorImage(
        cursorState.cursorType,
        cursorStyle.color,
        cursorStyle.borderColor
      );

  if (!image) return;

  const hotspot = hasCustomCursor
    ? { x: 0, y: 0 }
    : getCursorHotspot(cursorState.cursorType);

  const drawSpriteTo = (
    target: Context2D,
    originX: number,
    originY: number,
    alpha: number
  ): void => {
    target.save();
    target.globalAlpha = alpha;
    target.translate(originX, originY);
    target.scale(clickScale, clickScale);
    target.translate(-size * hotspot.x, -size * hotspot.y);
    target.drawImage(image, 0, 0, size, size);
    target.restore();
  };

  const { dx, dy } = cursorStyle.motionBlur
    ? getMotionBlurOffset(
        cursorData.events,
        videoTime,
        cursorStyle.smoothing,
        videoWidth,
        videoHeight,
        cursorStyle.motionBlurStrength
      )
    : { dx: 0, dy: 0 };

  if (Math.hypot(dx, dy) < MOTION_BLUR_MIN_TRAVEL) {
    drawSpriteTo(ctx, x, y, opacity);
    return;
  }

  const halfDx = Math.abs(dx) / 2;
  const halfDy = Math.abs(dy) / 2;
  const pad = Math.ceil(size * clickScale);
  const bufferWidth = Math.ceil(size * clickScale + halfDx * 2) + pad;
  const bufferHeight = Math.ceil(size * clickScale + halfDy * 2) + pad;

  const buffer = getMotionBlurBuffer(bufferWidth, bufferHeight);
  if (!buffer) {
    drawSpriteTo(ctx, x, y, opacity);
    return;
  }

  const localX = bufferWidth / 2;
  const localY = bufferHeight / 2;

  buffer.ctx.clearRect(0, 0, bufferWidth, bufferHeight);
  const center = (MOTION_BLUR_SAMPLES - 1) / 2;
  for (let index = 0; index < MOTION_BLUR_SAMPLES; index++) {
    const t = (index - center) / center;
    drawSpriteTo(
      buffer.ctx,
      localX + (dx * t) / 2,
      localY + (dy * t) / 2,
      1 / (index + 1)
    );
  }

  ctx.save();
  ctx.globalAlpha = opacity;
  ctx.drawImage(
    buffer.canvas,
    0,
    0,
    bufferWidth,
    bufferHeight,
    x - localX,
    y - localY,
    bufferWidth,
    bufferHeight
  );
  ctx.restore();
}
