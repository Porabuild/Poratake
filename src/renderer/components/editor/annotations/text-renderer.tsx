import type { TextAnnotation } from '@/types/editor';
import {
  measureText,
  TEXT_BG_PADDING_X,
  TEXT_BG_PADDING_Y,
  TEXT_BG_BORDER_RADIUS,
  SELECTION_BORDER_WIDTH,
  TEXT_FONT_WEIGHT,
} from '../text/text-utils';
import {
  type AnnotationRenderProps,
  type ExportRenderProps,
  type ResizeHandleRenderProps,
  SELECTION_STROKE,
} from './types';

interface TextRenderProps extends Omit<AnnotationRenderProps, 'annotation'> {
  annotation: TextAnnotation;
  editingTextId: string | null;
}

export function renderText({
  annotation: ann,
  offsetX,
  offsetY,
  isSelected,
  onMouseDown,
  onDoubleClick,
  editingTextId,
}: TextRenderProps): JSX.Element | null {
  if (ann.id === editingTextId || !ann.text) return null;

  const textX = ann.x + offsetX;
  const textY = ann.y + offsetY;
  const hasBackground = !!ann.backgroundColor;
  const bgPadding = hasBackground
    ? ann.backgroundPadding || { x: TEXT_BG_PADDING_X, y: TEXT_BG_PADDING_Y }
    : { x: 0, y: 0 };
  const bgRadius =
    ann.backgroundRadius !== undefined
      ? ann.backgroundRadius
      : TEXT_BG_BORDER_RADIUS;
  const fontFamily = ann.fontFamily || 'Arial, sans-serif';

  const measured = measureText(
    ann.text,
    ann.fontSize,
    fontFamily,
    TEXT_FONT_WEIGHT
  );
  const textWidth = measured.width;
  const textHeight = measured.height;

  const boxWidth = textWidth + bgPadding.x * 2;
  const boxHeight = textHeight + bgPadding.y * 2;

  const borderWidth = SELECTION_BORDER_WIDTH * 2;

  const centerX = textX - bgPadding.x + boxWidth / 2;
  const centerY = textY - bgPadding.y + boxHeight / 2;
  const rotation = ann.rotation || 0;

  return (
    <g
      key={ann.id}
      style={{
        cursor: 'move',
        userSelect: 'none',
        pointerEvents: 'auto',
      }}
      transform={`rotate(${rotation}, ${centerX}, ${centerY})`}
      onMouseDown={e => onMouseDown?.(e, ann.id)}
      onDoubleClick={e => onDoubleClick?.(e, ann.id)}
    >
      {isSelected && (
        <rect
          x={textX - bgPadding.x - borderWidth / 2}
          y={textY - bgPadding.y - borderWidth / 2}
          width={boxWidth + borderWidth}
          height={boxHeight + borderWidth}
          rx={Math.max(0, bgRadius - borderWidth / 2)}
          ry={Math.max(0, bgRadius - borderWidth / 2)}
          fill="none"
          stroke={SELECTION_STROKE}
          strokeWidth={borderWidth}
        />
      )}
      {hasBackground && (
        <rect
          x={textX - bgPadding.x}
          y={textY - bgPadding.y}
          width={boxWidth}
          height={boxHeight}
          rx={bgRadius}
          ry={bgRadius}
          fill={ann.backgroundColor}
        />
      )}
      <text
        x={textX}
        y={textY}
        fill={ann.fill}
        fontSize={ann.fontSize}
        fontFamily={fontFamily}
        fontWeight={TEXT_FONT_WEIGHT}
        dominantBaseline="text-before-edge"
      >
        {ann.text}
      </text>
    </g>
  );
}

interface TextHandlesProps extends Omit<ResizeHandleRenderProps, 'annotation'> {
  annotation: TextAnnotation;
}

export function renderTextHandles({
  annotation: ann,
  offsetX,
  offsetY,
  handleSize,
  onResizeStart,
}: TextHandlesProps): JSX.Element {
  const fontFamily = ann.fontFamily || 'Arial, sans-serif';
  const measured = measureText(
    ann.text,
    ann.fontSize,
    fontFamily,
    TEXT_FONT_WEIGHT
  );
  const hasBackground = !!ann.backgroundColor;
  const bgPadding = hasBackground
    ? ann.backgroundPadding || { x: TEXT_BG_PADDING_X, y: TEXT_BG_PADDING_Y }
    : { x: 0, y: 0 };

  const boxWidth = measured.width + bgPadding.x * 2;
  const boxHeight = measured.height + bgPadding.y * 2;

  const boxX = ann.x + offsetX - bgPadding.x;
  const boxY = ann.y + offsetY - bgPadding.y;

  const centerX = boxX + boxWidth / 2;
  const centerY = boxY + boxHeight / 2;

  const rotateHandleDistance = 24;
  const rotation = ann.rotation || 0;
  const rotationRad = (rotation * Math.PI) / 180;

  const handleDist = boxHeight / 2 + rotateHandleDistance;
  const rotateHandleX = centerX + Math.sin(rotationRad) * handleDist;
  const rotateHandleY = centerY - Math.cos(rotationRad) * handleDist;

  const cos = Math.cos(rotationRad);
  const sin = Math.sin(rotationRad);
  const halfW = boxWidth / 2;
  const halfH = boxHeight / 2;

  const brX = centerX + halfW * cos - halfH * sin;
  const brY = centerY + halfW * sin + halfH * cos;

  const handleStyle = {
    fill: 'white',
    stroke: '#007AFF',
    strokeWidth: 2,
    cursor: 'default',
    pointerEvents: 'auto' as const,
  };

  return (
    <>
      <line
        x1={centerX + Math.sin(rotationRad) * (boxHeight / 2)}
        y1={centerY - Math.cos(rotationRad) * (boxHeight / 2)}
        x2={rotateHandleX}
        y2={rotateHandleY}
        stroke="#007AFF"
        strokeWidth={1}
        strokeDasharray="4 2"
        style={{ pointerEvents: 'none' }}
      />
      <circle
        cx={rotateHandleX}
        cy={rotateHandleY}
        r={handleSize / 2}
        fill="#007AFF"
        stroke="white"
        strokeWidth={2}
        style={{ cursor: 'grab', pointerEvents: 'auto' }}
        onMouseDown={e => onResizeStart(e, ann.id, 'rotate')}
      />
      <rect
        x={brX - handleSize / 2}
        y={brY - handleSize / 2}
        width={handleSize}
        height={handleSize}
        rx={2}
        transform={`rotate(${rotation}, ${brX}, ${brY})`}
        {...handleStyle}
        style={{ cursor: 'nwse-resize', pointerEvents: 'auto' }}
        onMouseDown={e => onResizeStart(e, ann.id, 'bottom-right')}
      />
    </>
  );
}

interface TextExportProps extends Omit<ExportRenderProps, 'annotation'> {
  annotation: TextAnnotation;
}

export function exportText({
  annotation: ann,
  offsetX,
  offsetY,
  scale,
}: TextExportProps): string {
  if (!ann.text) return '';

  const textX = (ann.x + offsetX) * scale;
  const textY = (ann.y + offsetY) * scale;
  const hasBackground = !!ann.backgroundColor;
  const bgPadding = hasBackground
    ? ann.backgroundPadding || { x: TEXT_BG_PADDING_X, y: TEXT_BG_PADDING_Y }
    : { x: 0, y: 0 };
  const bgRadius =
    ann.backgroundRadius !== undefined
      ? ann.backgroundRadius
      : TEXT_BG_BORDER_RADIUS;
  const fontFamily = ann.fontFamily || 'Arial, sans-serif';
  const fontSize = ann.fontSize * scale;
  const scaledBgPadding = { x: bgPadding.x * scale, y: bgPadding.y * scale };
  const scaledBgRadius = bgRadius * scale;

  const measured = measureText(
    ann.text,
    ann.fontSize,
    fontFamily,
    TEXT_FONT_WEIGHT
  );
  const boxWidth = (measured.width + bgPadding.x * 2) * scale;
  const boxHeight = (measured.height + bgPadding.y * 2) * scale;

  const centerX = textX - scaledBgPadding.x + boxWidth / 2;
  const centerY = textY - scaledBgPadding.y + boxHeight / 2;
  const rotation = ann.rotation || 0;

  let result = `<g transform="rotate(${rotation}, ${centerX}, ${centerY})">`;
  if (hasBackground) {
    result += `<rect x="${textX - scaledBgPadding.x}" y="${textY - scaledBgPadding.y}" width="${boxWidth}" height="${boxHeight}" rx="${scaledBgRadius}" ry="${scaledBgRadius}" fill="${ann.backgroundColor}"/>`;
  }

  const escapedText = ann.text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
  // Escape double quotes in font-family for valid SVG/XML attributes
  const escapedFontFamily = fontFamily.replace(/"/g, '&quot;');
  result += `<text x="${textX}" y="${textY}" fill="${ann.fill}" font-size="${fontSize}" font-family="${escapedFontFamily}" font-weight="${TEXT_FONT_WEIGHT}" dominant-baseline="text-before-edge">${escapedText}</text>`;
  result += '</g>';

  return result;
}
