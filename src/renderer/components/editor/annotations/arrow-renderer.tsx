import type { JSX } from 'react';
import type { ArrowAnnotation, ArrowStyle } from '@/types/editor';
import {
  type AnnotationRenderProps,
  type ExportRenderProps,
  type ResizeHandleRenderProps,
  type ResizeHandle,
  SELECTION_STROKE,
  SELECTION_STROKE_WIDTH,
} from './types';

interface ArrowPathResult {
  linePath: string;
  headPath: string;
  isPolygonHead: boolean;
}

export const renderArrowPath = (
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  strokeWidth: number,
  style: ArrowStyle = 'standard',
  bendOffset?: { x: number; y: number }
): ArrowPathResult => {
  const headSize = Math.max(16, strokeWidth * 5);

  const midX = (x1 + x2) / 2;
  const midY = (y1 + y2) / 2;
  const ctrlX = midX + (bendOffset?.x || 0);
  const ctrlY = midY + (bendOffset?.y || 0);

  const hasBend =
    bendOffset && (Math.abs(bendOffset.x) > 1 || Math.abs(bendOffset.y) > 1);

  const angle = hasBend
    ? Math.atan2(y2 - ctrlY, x2 - ctrlX)
    : Math.atan2(y2 - y1, x2 - x1);

  switch (style) {
    case 'standard': {
      const headAngle = Math.PI / 6;
      const leftX = x2 - headSize * Math.cos(angle - headAngle);
      const leftY = y2 - headSize * Math.sin(angle - headAngle);
      const rightX = x2 - headSize * Math.cos(angle + headAngle);
      const rightY = y2 - headSize * Math.sin(angle + headAngle);

      const linePath = hasBend
        ? `M ${x1} ${y1} Q ${ctrlX} ${ctrlY} ${x2} ${y2}`
        : `M ${x1} ${y1} L ${x2} ${y2}`;

      return {
        linePath,
        headPath: `M ${x2} ${y2} L ${leftX} ${leftY} M ${x2} ${y2} L ${rightX} ${rightY}`,
        isPolygonHead: false,
      };
    }

    case 'curved': {
      const distance = Math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2);
      let curveCtrlX = ctrlX;
      let curveCtrlY = ctrlY;

      if (!hasBend) {
        const curveOffset = distance * 0.2;
        const perpX = -(y2 - y1) / (distance || 1);
        const perpY = (x2 - x1) / (distance || 1);
        curveCtrlX = midX + perpX * curveOffset;
        curveCtrlY = midY + perpY * curveOffset;
      }

      const endAngle = Math.atan2(y2 - curveCtrlY, x2 - curveCtrlX);
      const headAngle = Math.PI / 6;
      const leftX = x2 - headSize * Math.cos(endAngle - headAngle);
      const leftY = y2 - headSize * Math.sin(endAngle - headAngle);
      const rightX = x2 - headSize * Math.cos(endAngle + headAngle);
      const rightY = y2 - headSize * Math.sin(endAngle + headAngle);

      return {
        linePath: `M ${x1} ${y1} Q ${curveCtrlX} ${curveCtrlY} ${x2} ${y2}`,
        headPath: `M ${x2} ${y2} L ${leftX} ${leftY} M ${x2} ${y2} L ${rightX} ${rightY}`,
        isPolygonHead: false,
      };
    }

    case 'double': {
      const headAngle = Math.PI / 6;

      const endLeftX = x2 - headSize * Math.cos(angle - headAngle);
      const endLeftY = y2 - headSize * Math.sin(angle - headAngle);
      const endRightX = x2 - headSize * Math.cos(angle + headAngle);
      const endRightY = y2 - headSize * Math.sin(angle + headAngle);

      const startAngle = hasBend
        ? Math.atan2(y1 - ctrlY, x1 - ctrlX)
        : Math.atan2(y1 - y2, x1 - x2);
      const startLeftX = x1 - headSize * Math.cos(startAngle - headAngle);
      const startLeftY = y1 - headSize * Math.sin(startAngle - headAngle);
      const startRightX = x1 - headSize * Math.cos(startAngle + headAngle);
      const startRightY = y1 - headSize * Math.sin(startAngle + headAngle);

      const linePath = hasBend
        ? `M ${x1} ${y1} Q ${ctrlX} ${ctrlY} ${x2} ${y2}`
        : `M ${x1} ${y1} L ${x2} ${y2}`;

      return {
        linePath,
        headPath: `M ${x2} ${y2} L ${endLeftX} ${endLeftY} M ${x2} ${y2} L ${endRightX} ${endRightY} M ${x1} ${y1} L ${startLeftX} ${startLeftY} M ${x1} ${y1} L ${startRightX} ${startRightY}`,
        isPolygonHead: false,
      };
    }

    case 'double-curved': {
      const distance = Math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2);
      let curveCtrlX = ctrlX;
      let curveCtrlY = ctrlY;

      if (!hasBend) {
        const curveOffset = distance * 0.2;
        const perpX = -(y2 - y1) / (distance || 1);
        const perpY = (x2 - x1) / (distance || 1);
        curveCtrlX = midX + perpX * curveOffset;
        curveCtrlY = midY + perpY * curveOffset;
      }

      const endAngle = Math.atan2(y2 - curveCtrlY, x2 - curveCtrlX);
      const headAngle = Math.PI / 6;
      const endLeftX = x2 - headSize * Math.cos(endAngle - headAngle);
      const endLeftY = y2 - headSize * Math.sin(endAngle - headAngle);
      const endRightX = x2 - headSize * Math.cos(endAngle + headAngle);
      const endRightY = y2 - headSize * Math.sin(endAngle + headAngle);

      const startAngle = Math.atan2(y1 - curveCtrlY, x1 - curveCtrlX);
      const startLeftX = x1 - headSize * Math.cos(startAngle - headAngle);
      const startLeftY = y1 - headSize * Math.sin(startAngle - headAngle);
      const startRightX = x1 - headSize * Math.cos(startAngle + headAngle);
      const startRightY = y1 - headSize * Math.sin(startAngle + headAngle);

      return {
        linePath: `M ${x1} ${y1} Q ${curveCtrlX} ${curveCtrlY} ${x2} ${y2}`,
        headPath: `M ${x2} ${y2} L ${endLeftX} ${endLeftY} M ${x2} ${y2} L ${endRightX} ${endRightY} M ${x1} ${y1} L ${startLeftX} ${startLeftY} M ${x1} ${y1} L ${startRightX} ${startRightY}`,
        isPolygonHead: false,
      };
    }

    default:
      return {
        linePath: hasBend
          ? `M ${x1} ${y1} Q ${ctrlX} ${ctrlY} ${x2} ${y2}`
          : `M ${x1} ${y1} L ${x2} ${y2}`,
        headPath: '',
        isPolygonHead: false,
      };
  }
};

interface ArrowRenderProps extends Omit<AnnotationRenderProps, 'annotation'> {
  annotation: ArrowAnnotation;
}

export function renderArrow({
  annotation: ann,
  offsetX,
  offsetY,
  isSelected,
  isPreview,
  onMouseDown,
}: ArrowRenderProps): JSX.Element {
  const key = isPreview ? `preview-${ann.id}` : ann.id;
  const [x1, y1, x2, y2] = ann.points;
  const ax1 = x1 + offsetX;
  const ay1 = y1 + offsetY;
  const ax2 = x2 + offsetX;
  const ay2 = y2 + offsetY;
  const arrowStyle = ann.arrowStyle || 'standard';
  const { linePath, headPath, isPolygonHead } = renderArrowPath(
    ax1,
    ay1,
    ax2,
    ay2,
    ann.strokeWidth,
    arrowStyle,
    ann.bendOffset
  );

  return (
    <g
      key={key}
      style={{
        cursor: isPreview ? 'default' : 'move',
        pointerEvents: isPreview ? 'none' : 'auto',
      }}
      onMouseDown={isPreview ? undefined : e => onMouseDown?.(e, ann.id)}
    >
      {isSelected && (
        <>
          <path
            d={linePath}
            fill="none"
            stroke={SELECTION_STROKE}
            strokeWidth={ann.strokeWidth + SELECTION_STROKE_WIDTH}
            strokeLinecap="round"
          />
          {headPath && (
            <path
              d={headPath}
              fill={isPolygonHead ? SELECTION_STROKE : 'none'}
              stroke={SELECTION_STROKE}
              strokeWidth={isPolygonHead ? 2 : ann.strokeWidth + 4}
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          )}
        </>
      )}
      <path
        d={linePath}
        fill="none"
        stroke={ann.stroke}
        strokeWidth={ann.strokeWidth}
        strokeLinecap="round"
      />
      {headPath && (
        <path
          d={headPath}
          fill={isPolygonHead ? ann.stroke : 'none'}
          stroke={ann.stroke}
          strokeWidth={isPolygonHead ? 1 : ann.strokeWidth}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      )}
    </g>
  );
}

interface ArrowHandlesProps extends Omit<
  ResizeHandleRenderProps,
  'annotation'
> {
  annotation: ArrowAnnotation;
}

export function renderArrowHandles({
  annotation: ann,
  offsetX,
  offsetY,
  handleSize,
  onResizeStart,
}: ArrowHandlesProps): JSX.Element {
  const [x1, y1, x2, y2] = ann.points;
  const midX = (x1 + x2) / 2 + offsetX;
  const midY = (y1 + y2) / 2 + offsetY;
  const bendX = midX + (ann.bendOffset?.x || 0);
  const bendY = midY + (ann.bendOffset?.y || 0);

  const handles: {
    pos: ResizeHandle;
    cx: number;
    cy: number;
    isBend?: boolean;
  }[] = [
    { pos: 'start', cx: x1 + offsetX, cy: y1 + offsetY },
    { pos: 'end', cx: x2 + offsetX, cy: y2 + offsetY },
    { pos: 'bend', cx: bendX, cy: bendY, isBend: true },
  ];

  return (
    <>
      {}
      <line
        x1={midX}
        y1={midY}
        x2={bendX}
        y2={bendY}
        stroke="#007AFF"
        strokeWidth={1}
        strokeDasharray="4 2"
        style={{ pointerEvents: 'none' }}
      />
      {handles.map(({ pos, cx, cy, isBend }) => (
        <circle
          key={`handle-${ann.id}-${pos}`}
          cx={cx}
          cy={cy}
          r={isBend ? handleSize / 2 - 1 : handleSize / 2}
          fill={isBend ? '#007AFF' : 'white'}
          stroke={isBend ? 'white' : '#007AFF'}
          strokeWidth={2}
          style={{ cursor: 'move', pointerEvents: 'auto' }}
          onMouseDown={e => onResizeStart(e, ann.id, pos)}
        />
      ))}
    </>
  );
}

interface ArrowExportProps extends Omit<ExportRenderProps, 'annotation'> {
  annotation: ArrowAnnotation;
}

export function exportArrow({
  annotation: ann,
  offsetX,
  offsetY,
  scale,
}: ArrowExportProps): string {
  const [x1, y1, x2, y2] = ann.points;
  const ax1 = (x1 + offsetX) * scale;
  const ay1 = (y1 + offsetY) * scale;
  const ax2 = (x2 + offsetX) * scale;
  const ay2 = (y2 + offsetY) * scale;
  const arrowStyle = ann.arrowStyle || 'standard';

  const scaledBendOffset = ann.bendOffset
    ? { x: ann.bendOffset.x * scale, y: ann.bendOffset.y * scale }
    : undefined;

  const { linePath, headPath, isPolygonHead } = renderArrowPath(
    ax1,
    ay1,
    ax2,
    ay2,
    ann.strokeWidth * scale,
    arrowStyle,
    scaledBendOffset
  );

  let result = `<path d="${linePath}" fill="none" stroke="${ann.stroke}" stroke-width="${ann.strokeWidth * scale}" stroke-linecap="round"/>`;
  if (headPath) {
    result += `<path d="${headPath}" fill="${isPolygonHead ? ann.stroke : 'none'}" stroke="${ann.stroke}" stroke-width="${isPolygonHead ? 1 : ann.strokeWidth * scale}" stroke-linecap="round" stroke-linejoin="round"/>`;
  }
  return result;
}
