import { getStroke } from 'perfect-freehand';
import type {
  Annotation,
  ArrowAnnotation,
  CircleAnnotation,
  HighlightAnnotation,
  LineAnnotation,
  NumberAnnotation,
  PenAnnotation,
  RectAnnotation,
  RedactAnnotation,
  TextAnnotation,
} from '@/types/editor';
import type { DrawingSegment } from '@/types/drawing';
import { TEXT_FONT_WEIGHT } from '@/renderer/components/editor/text/text-utils';
import { REDACT_INTENSITY_MAP } from '@/renderer/utils/redact';
import { scaleAnnotationToComposition } from './drawing-scale';
import type { Context2D } from './types';
import { getContrastColor } from '@/renderer/utils/color';
import { pointsToCoordinates } from '@/renderer/utils/annotation-geometry';
import { normalizeNegativeRect } from '@/renderer/utils/annotation-geometry';
import {
  NUMBER_SIZE_CONFIG,
  arrowHeadPoints,
  arrowHeadSize,
  curvedControlPoint,
  hasArrowBend,
} from '@/renderer/utils/annotation-geometry';

interface RenderDrawingsOptions {
  drawingSegments?: DrawingSegment[] | null;
  timelineTime: number;
  width: number;
  height: number;
  onlyRedact?: boolean;
}

const DEFAULT_TEXT_FONT = 'Arial, sans-serif';

function drawFreehandPath(ctx: Context2D, points: number[][]): void {
  if (points.length === 0) return;

  ctx.beginPath();
  ctx.moveTo(points[0][0], points[0][1]);

  for (let i = 0; i < points.length; i++) {
    const [x0, y0] = points[i];
    const [x1, y1] = points[(i + 1) % points.length];
    ctx.quadraticCurveTo(x0, y0, (x0 + x1) / 2, (y0 + y1) / 2);
  }

  ctx.closePath();
}

function renderPen(ctx: Context2D, annotation: PenAnnotation): void {
  const coords = pointsToCoordinates(annotation.points);
  if (coords.length === 0) return;

  const outlinePoints = getStroke(coords, {
    size: annotation.strokeWidth * 2,
    thinning: 0.5,
    smoothing: 0.6,
    streamline: 0.5,
    simulatePressure: true,
  });

  ctx.save();
  ctx.fillStyle = annotation.stroke;
  drawFreehandPath(ctx, outlinePoints);
  ctx.fill();
  ctx.restore();
}

function renderHighlight(
  ctx: Context2D,
  annotation: HighlightAnnotation
): void {
  const coords = pointsToCoordinates(annotation.points);
  if (coords.length < 2) return;

  const halfWidth = annotation.strokeWidth / 2;
  const upperEdge: [number, number][] = [];
  const lowerEdge: [number, number][] = [];

  for (let i = 0; i < coords.length; i++) {
    const [x, y] = coords[i];
    let dx = 0;
    let dy = 1;

    if (i < coords.length - 1) {
      const [nextX, nextY] = coords[i + 1];
      const length = Math.sqrt((nextX - x) ** 2 + (nextY - y) ** 2);
      if (length > 0) {
        dx = -(nextY - y) / length;
        dy = (nextX - x) / length;
      }
    } else if (i > 0) {
      const [prevX, prevY] = coords[i - 1];
      const length = Math.sqrt((x - prevX) ** 2 + (y - prevY) ** 2);
      if (length > 0) {
        dx = -(y - prevY) / length;
        dy = (x - prevX) / length;
      }
    }

    upperEdge.push([x + dx * halfWidth, y + dy * halfWidth]);
    lowerEdge.push([x - dx * halfWidth, y - dy * halfWidth]);
  }

  ctx.save();
  ctx.globalCompositeOperation = 'multiply';
  ctx.globalAlpha = annotation.opacity;
  ctx.fillStyle = annotation.fill;
  ctx.beginPath();
  ctx.moveTo(upperEdge[0][0], upperEdge[0][1]);

  for (let i = 1; i < upperEdge.length; i++) {
    ctx.lineTo(upperEdge[i][0], upperEdge[i][1]);
  }

  for (let i = lowerEdge.length - 1; i >= 0; i--) {
    ctx.lineTo(lowerEdge[i][0], lowerEdge[i][1]);
  }

  ctx.closePath();
  ctx.fill();
  ctx.restore();
}

function renderRectangle(ctx: Context2D, annotation: RectAnnotation): void {
  const rect = normalizeNegativeRect(
    annotation.x,
    annotation.y,
    annotation.width,
    annotation.height
  );

  ctx.save();
  ctx.lineWidth = annotation.strokeWidth;
  ctx.strokeStyle = annotation.stroke;
  ctx.fillStyle = annotation.fill ?? 'transparent';
  ctx.beginPath();
  ctx.roundRect(rect.x, rect.y, rect.width, rect.height, 1);
  if (annotation.fill) {
    ctx.fill();
  }
  ctx.stroke();
  ctx.restore();
}

function renderCircle(ctx: Context2D, annotation: CircleAnnotation): void {
  ctx.save();
  ctx.lineWidth = annotation.strokeWidth;
  ctx.strokeStyle = annotation.stroke;
  ctx.fillStyle = annotation.fill ?? 'transparent';
  ctx.beginPath();
  ctx.arc(annotation.x, annotation.y, annotation.radius, 0, Math.PI * 2);
  if (annotation.fill) {
    ctx.fill();
  }
  ctx.stroke();
  ctx.restore();
}

function renderLine(ctx: Context2D, annotation: LineAnnotation): void {
  const [x1, y1, x2, y2] = annotation.points;

  ctx.save();
  ctx.lineWidth = annotation.strokeWidth;
  ctx.strokeStyle = annotation.stroke;
  ctx.lineCap = 'round';
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x2, y2);
  ctx.stroke();
  ctx.restore();
}

function getArrowControlPoint(annotation: ArrowAnnotation): {
  x: number;
  y: number;
  hasCurve: boolean;
} {
  const [x1, y1, x2, y2] = annotation.points;
  const midX = (x1 + x2) / 2;
  const midY = (y1 + y2) / 2;
  const hasBend = hasArrowBend(annotation.bendOffset);

  if (hasBend) {
    return {
      x: midX + (annotation.bendOffset?.x ?? 0),
      y: midY + (annotation.bendOffset?.y ?? 0),
      hasCurve: true,
    };
  }

  const style = annotation.arrowStyle ?? 'standard';
  if (style !== 'curved' && style !== 'double-curved') {
    return { x: midX, y: midY, hasCurve: false };
  }

  return { ...curvedControlPoint(x1, y1, x2, y2), hasCurve: true };
}

function drawArrowHead(
  ctx: Context2D,
  x: number,
  y: number,
  angle: number,
  headSize: number
): void {
  const { leftX, leftY, rightX, rightY } = arrowHeadPoints(
    x,
    y,
    angle,
    headSize
  );

  ctx.beginPath();
  ctx.moveTo(x, y);
  ctx.lineTo(leftX, leftY);
  ctx.moveTo(x, y);
  ctx.lineTo(rightX, rightY);
  ctx.stroke();
}

function renderArrow(ctx: Context2D, annotation: ArrowAnnotation): void {
  const [x1, y1, x2, y2] = annotation.points;
  const control = getArrowControlPoint(annotation);
  const style = annotation.arrowStyle ?? 'standard';
  const headSize = arrowHeadSize(annotation.strokeWidth);

  ctx.save();
  ctx.lineWidth = annotation.strokeWidth;
  ctx.strokeStyle = annotation.stroke;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  ctx.beginPath();
  ctx.moveTo(x1, y1);
  if (control.hasCurve) {
    ctx.quadraticCurveTo(control.x, control.y, x2, y2);
  } else {
    ctx.lineTo(x2, y2);
  }
  ctx.stroke();

  const endAngle = control.hasCurve
    ? Math.atan2(y2 - control.y, x2 - control.x)
    : Math.atan2(y2 - y1, x2 - x1);
  drawArrowHead(ctx, x2, y2, endAngle, headSize);

  if (style === 'double' || style === 'double-curved') {
    const startAngle = control.hasCurve
      ? Math.atan2(y1 - control.y, x1 - control.x)
      : Math.atan2(y1 - y2, x1 - x2);
    drawArrowHead(ctx, x1, y1, startAngle, headSize);
  }

  ctx.restore();
}

function renderText(ctx: Context2D, annotation: TextAnnotation): void {
  if (!annotation.text) return;

  const fontFamily = annotation.fontFamily || DEFAULT_TEXT_FONT;
  const padding = annotation.backgroundPadding ?? { x: 0, y: 0 };
  const radius = annotation.backgroundRadius ?? 4;

  ctx.save();
  ctx.font = `${TEXT_FONT_WEIGHT} ${annotation.fontSize}px ${fontFamily}`;
  ctx.textBaseline = 'alphabetic';

  const metrics = ctx.measureText(annotation.text);
  const ascent =
    metrics.fontBoundingBoxAscent ??
    metrics.actualBoundingBoxAscent ??
    annotation.fontSize;
  const descent =
    metrics.fontBoundingBoxDescent ?? metrics.actualBoundingBoxDescent ?? 0;
  const textHeight = ascent + descent || annotation.fontSize;
  const boxWidth = metrics.width + padding.x * 2;
  const boxHeight = textHeight + padding.y * 2;
  const centerX = annotation.x - padding.x + boxWidth / 2;
  const centerY = annotation.y - padding.y + boxHeight / 2;
  const rotation = ((annotation.rotation ?? 0) * Math.PI) / 180;

  ctx.translate(centerX, centerY);
  ctx.rotate(rotation);
  ctx.translate(-centerX, -centerY);

  if (annotation.backgroundColor) {
    ctx.fillStyle = annotation.backgroundColor;
    ctx.beginPath();
    ctx.roundRect(
      annotation.x - padding.x,
      annotation.y - padding.y,
      boxWidth,
      boxHeight,
      radius
    );
    ctx.fill();
  }

  ctx.fillStyle = annotation.fill;
  ctx.fillText(annotation.text, annotation.x, annotation.y + ascent);
  ctx.restore();
}

function renderNumber(ctx: Context2D, annotation: NumberAnnotation): void {
  const config = NUMBER_SIZE_CONFIG[annotation.size];
  const textColor = getContrastColor(annotation.fill);

  ctx.save();
  ctx.fillStyle = annotation.fill;
  ctx.beginPath();
  ctx.arc(annotation.x, annotation.y, config.radius, 0, Math.PI * 2);
  ctx.fill();

  ctx.fillStyle = textColor;
  ctx.font = `700 ${config.fontSize}px system-ui, -apple-system, sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(annotation.displayValue, annotation.x, annotation.y);
  ctx.restore();
}

function getDeviceScale(ctx: Context2D): { x: number; y: number } {
  const transform = ctx.getTransform();
  return {
    x: transform.a || 1,
    y: transform.d || 1,
  };
}

let scratchCanvas: OffscreenCanvas | null = null;
let scratchCtx: OffscreenCanvasRenderingContext2D | null = null;

function getScratchContext(): OffscreenCanvasRenderingContext2D | null {
  if (!scratchCanvas) {
    scratchCanvas = new OffscreenCanvas(1, 1);
    scratchCtx = scratchCanvas.getContext('2d');
  }
  return scratchCtx;
}

interface DeviceRegion {
  device: { x: number; y: number; width: number; height: number };
  composition: { x: number; y: number; width: number; height: number };
}

function clampRegion(
  ctx: Context2D,
  x: number,
  y: number,
  width: number,
  height: number
): DeviceRegion | null {
  const scale = getDeviceScale(ctx);
  const flooredX = Math.floor(x * scale.x);
  const flooredY = Math.floor(y * scale.y);
  const clampedX = Math.max(0, flooredX);
  const clampedY = Math.max(0, flooredY);
  const clampedWidth = Math.min(
    Math.ceil(width * scale.x) - (clampedX - flooredX),
    ctx.canvas.width - clampedX
  );
  const clampedHeight = Math.min(
    Math.ceil(height * scale.y) - (clampedY - flooredY),
    ctx.canvas.height - clampedY
  );

  if (clampedWidth <= 0 || clampedHeight <= 0) return null;

  return {
    device: {
      x: clampedX,
      y: clampedY,
      width: clampedWidth,
      height: clampedHeight,
    },
    composition: {
      x: clampedX / scale.x,
      y: clampedY / scale.y,
      width: clampedWidth / scale.x,
      height: clampedHeight / scale.y,
    },
  };
}

function pixelateRegion(
  ctx: Context2D,
  x: number,
  y: number,
  width: number,
  height: number,
  blockSize: number
): void {
  const region = clampRegion(ctx, x, y, width, height);
  if (!region) return;

  const scratch = getScratchContext();
  if (!scratch || !scratchCanvas) return;

  const { device, composition } = region;
  const scale = getDeviceScale(ctx);
  const deviceBlockWidth = Math.max(1, blockSize * scale.x);
  const deviceBlockHeight = Math.max(1, blockSize * scale.y);
  const smallWidth = Math.max(1, Math.round(device.width / deviceBlockWidth));
  const smallHeight = Math.max(
    1,
    Math.round(device.height / deviceBlockHeight)
  );

  scratchCanvas.width = smallWidth;
  scratchCanvas.height = smallHeight;

  scratch.imageSmoothingEnabled = true;
  scratch.clearRect(0, 0, smallWidth, smallHeight);
  scratch.drawImage(
    ctx.canvas,
    device.x,
    device.y,
    device.width,
    device.height,
    0,
    0,
    smallWidth,
    smallHeight
  );

  ctx.save();
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(
    scratchCanvas,
    0,
    0,
    smallWidth,
    smallHeight,
    composition.x,
    composition.y,
    composition.width,
    composition.height
  );
  ctx.restore();
}

let blurCanvas: OffscreenCanvas | null = null;
let blurCtx: OffscreenCanvasRenderingContext2D | null = null;

function getBlurContext(): OffscreenCanvasRenderingContext2D | null {
  if (!blurCanvas) {
    blurCanvas = new OffscreenCanvas(1, 1);
    blurCtx = blurCanvas.getContext('2d');
  }
  return blurCtx;
}

function blurRegion(
  ctx: Context2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number
): void {
  const region = clampRegion(ctx, x, y, width, height);
  if (!region) return;

  const blur = getBlurContext();
  if (!blur || !blurCanvas) return;

  const { device, composition } = region;
  const scale = getDeviceScale(ctx);
  const padding = radius * 2;
  const sourceX = Math.max(0, device.x - padding);
  const sourceY = Math.max(0, device.y - padding);
  const sourceWidth = Math.min(
    device.width + padding * 2,
    ctx.canvas.width - sourceX
  );
  const sourceHeight = Math.min(
    device.height + padding * 2,
    ctx.canvas.height - sourceY
  );

  blurCanvas.width = sourceWidth;
  blurCanvas.height = sourceHeight;

  blur.clearRect(0, 0, sourceWidth, sourceHeight);
  blur.drawImage(
    ctx.canvas,
    sourceX,
    sourceY,
    sourceWidth,
    sourceHeight,
    0,
    0,
    sourceWidth,
    sourceHeight
  );

  ctx.save();
  ctx.beginPath();
  ctx.rect(composition.x, composition.y, composition.width, composition.height);
  ctx.clip();
  ctx.filter = `blur(${radius / scale.x}px)`;
  ctx.drawImage(
    blurCanvas,
    0,
    0,
    sourceWidth,
    sourceHeight,
    sourceX / scale.x,
    sourceY / scale.y,
    sourceWidth / scale.x,
    sourceHeight / scale.y
  );
  ctx.filter = 'none';
  ctx.restore();
}

function renderRedact(ctx: Context2D, annotation: RedactAnnotation): void {
  const rect = normalizeNegativeRect(
    annotation.x,
    annotation.y,
    annotation.width,
    annotation.height
  );
  const intensity =
    REDACT_INTENSITY_MAP[annotation.intensity] ?? REDACT_INTENSITY_MAP[5];

  switch (annotation.style) {
    case 'pixelate':
      pixelateRegion(
        ctx,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        Math.max(1, Math.round(intensity.pixelSize))
      );
      return;
    case 'blur':
      blurRegion(
        ctx,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        Math.max(1, Math.round(intensity.blurRadius))
      );
      return;
    case 'blackout':
      ctx.save();
      ctx.fillStyle = '#000000';
      ctx.fillRect(rect.x, rect.y, rect.width, rect.height);
      ctx.restore();
  }
}

function renderAnnotation(ctx: Context2D, annotation: Annotation): void {
  switch (annotation.type) {
    case 'pen':
      renderPen(ctx, annotation);
      return;
    case 'highlight':
      renderHighlight(ctx, annotation);
      return;
    case 'rectangle':
      renderRectangle(ctx, annotation);
      return;
    case 'circle':
      renderCircle(ctx, annotation);
      return;
    case 'line':
      renderLine(ctx, annotation);
      return;
    case 'arrow':
      renderArrow(ctx, annotation);
      return;
    case 'text':
      renderText(ctx, annotation);
      return;
    case 'number':
      renderNumber(ctx, annotation);
      return;
    case 'redact':
      renderRedact(ctx, annotation);
  }
}

export function renderDrawings(
  ctx: Context2D,
  {
    drawingSegments,
    timelineTime,
    width,
    height,
    onlyRedact = false,
  }: RenderDrawingsOptions
): void {
  if (!drawingSegments || drawingSegments.length === 0) return;

  for (const segment of drawingSegments) {
    if (segment.canvasWidth <= 0 || segment.canvasHeight <= 0) continue;

    const annotations = getRenderableAnnotations(
      segment,
      timelineTime,
      onlyRedact
    );
    if (annotations.length === 0) continue;

    const scaleX = width / segment.canvasWidth;
    const scaleY = height / segment.canvasHeight;

    for (const annotation of annotations) {
      renderAnnotation(
        ctx,
        scaleAnnotationToComposition(annotation, scaleX, scaleY)
      );
    }
  }
}

export function getRenderableAnnotations(
  segment: DrawingSegment,
  timelineTime: number,
  onlyRedact: boolean
): Annotation[] {
  if (timelineTime < segment.startTime || timelineTime > segment.endTime) {
    return [];
  }

  if (!onlyRedact) return segment.annotations;

  return segment.annotations.filter(annotation => annotation.type === 'redact');
}
