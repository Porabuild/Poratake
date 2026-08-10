import type { RedactAnnotation } from '@/types/editor';
import {
  type AnnotationRenderProps,
  type ExportRenderProps,
  type ResizeHandleRenderProps,
  type ResizeHandle,
  SELECTION_STROKE,
  SELECTION_STROKE_WIDTH,
} from '../annotations/types';

interface RedactRenderProps extends Omit<AnnotationRenderProps, 'annotation'> {
  annotation: RedactAnnotation;
}

export function renderRedact({
  annotation: ann,
  offsetX,
  offsetY,
  isSelected,
  isPreview,
  onMouseDown,
}: RedactRenderProps): JSX.Element {
  const key = isPreview ? `preview-${ann.id}` : ann.id;
  const x = ann.x + offsetX;
  const y = ann.y + offsetY;
  const w = ann.width;
  const h = ann.height;
  const rectX = w < 0 ? x + w : x;
  const rectY = h < 0 ? y + h : y;
  const rectW = Math.abs(w);
  const rectH = Math.abs(h);

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
        <rect
          x={rectX}
          y={rectY}
          width={rectW}
          height={rectH}
          fill="none"
          stroke={SELECTION_STROKE}
          strokeWidth={SELECTION_STROKE_WIDTH}
          rx="2"
        />
      )}

      <rect
        x={rectX}
        y={rectY}
        width={rectW}
        height={rectH}
        fill="transparent"
        stroke={isSelected ? 'none' : 'rgba(100, 100, 100, 0.3)'}
        strokeWidth={1}
        strokeDasharray="4 2"
        rx="2"
      />
    </g>
  );
}

interface RedactHandlesProps extends Omit<
  ResizeHandleRenderProps,
  'annotation'
> {
  annotation: RedactAnnotation;
}

export function renderRedactHandles({
  annotation: ann,
  offsetX,
  offsetY,
  handleSize,
  onResizeStart,
}: RedactHandlesProps): JSX.Element {
  const x = ann.x + offsetX;
  const y = ann.y + offsetY;
  const w = ann.width;
  const h = ann.height;
  const rectX = w < 0 ? x + w : x;
  const rectY = h < 0 ? y + h : y;
  const rectW = Math.abs(w);
  const rectH = Math.abs(h);

  const handleStyle = {
    fill: 'white',
    stroke: '#007AFF',
    strokeWidth: 2,
    cursor: 'default',
    pointerEvents: 'auto' as const,
  };

  const handles: { pos: ResizeHandle; cx: number; cy: number }[] = [
    { pos: 'top-left', cx: rectX, cy: rectY },
    { pos: 'top-right', cx: rectX + rectW, cy: rectY },
    { pos: 'bottom-left', cx: rectX, cy: rectY + rectH },
    { pos: 'bottom-right', cx: rectX + rectW, cy: rectY + rectH },
  ];

  return (
    <>
      {handles.map(({ pos, cx, cy }) => (
        <rect
          key={`handle-${ann.id}-${pos}`}
          x={cx - handleSize / 2}
          y={cy - handleSize / 2}
          width={handleSize}
          height={handleSize}
          rx={2}
          {...handleStyle}
          style={{
            cursor:
              pos === 'top-left' || pos === 'bottom-right'
                ? 'nwse-resize'
                : 'nesw-resize',
          }}
          onMouseDown={e => onResizeStart(e, ann.id, pos)}
        />
      ))}
    </>
  );
}

interface RedactExportProps extends Omit<ExportRenderProps, 'annotation'> {
  annotation: RedactAnnotation;
}

export function exportRedact({
  annotation: ann,
  offsetX,
  offsetY,
  scale,
}: RedactExportProps): string {
  const x = (ann.x + offsetX) * scale;
  const y = (ann.y + offsetY) * scale;
  const w = ann.width * scale;
  const h = ann.height * scale;
  const rectX = w < 0 ? x + w : x;
  const rectY = h < 0 ? y + h : y;
  const rectW = Math.abs(w);
  const rectH = Math.abs(h);

  if (ann.style === 'blackout') {
    return `<rect x="${rectX}" y="${rectY}" width="${rectW}" height="${rectH}" fill="#000000" rx="2"/>`;
  }

  return '';
}
