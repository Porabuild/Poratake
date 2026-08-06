import type { NumberAnnotation, NumberSize } from '@/types/editor';
import {
  type AnnotationRenderProps,
  type ExportRenderProps,
  SELECTION_STROKE,
  SELECTION_STROKE_WIDTH,
  getContrastColor,
} from '../shared';

const SIZE_CONFIG: Record<NumberSize, { radius: number; fontSize: number }> = {
  small: { radius: 14, fontSize: 14 },
  medium: { radius: 18, fontSize: 18 },
  large: { radius: 24, fontSize: 24 },
};

interface NumberRenderProps extends Omit<AnnotationRenderProps, 'annotation'> {
  annotation: NumberAnnotation;
}

export function renderNumber({
  annotation: ann,
  offsetX,
  offsetY,
  isSelected,
  isPreview,
  onMouseDown,
}: NumberRenderProps): JSX.Element {
  const key = isPreview ? `preview-${ann.id}` : ann.id;
  const config = SIZE_CONFIG[ann.size] || SIZE_CONFIG.medium;
  const cx = ann.x + offsetX;
  const cy = ann.y + offsetY;
  const textColor = getContrastColor(ann.fill);

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
        <circle
          cx={cx}
          cy={cy}
          r={config.radius + 3}
          fill="none"
          stroke={SELECTION_STROKE}
          strokeWidth={SELECTION_STROKE_WIDTH}
        />
      )}
      <circle cx={cx} cy={cy} r={config.radius} fill={ann.fill} />
      <text
        x={cx}
        y={cy}
        textAnchor="middle"
        dominantBaseline="central"
        fontSize={config.fontSize}
        fontWeight="bold"
        fontFamily="system-ui, -apple-system, sans-serif"
        fill={textColor}
        style={{
          cursor: 'move',
          userSelect: 'none',
          pointerEvents: 'none',
        }}
      >
        {ann.displayValue}
      </text>
    </g>
  );
}

interface NumberExportProps extends Omit<ExportRenderProps, 'annotation'> {
  annotation: NumberAnnotation;
}

export function exportNumber({
  annotation: ann,
  offsetX,
  offsetY,
  scale,
}: NumberExportProps): string {
  const config = SIZE_CONFIG[ann.size] || SIZE_CONFIG.medium;
  const cx = (ann.x + offsetX) * scale;
  const cy = (ann.y + offsetY) * scale;
  const radius = config.radius * scale;
  const fontSize = config.fontSize * scale;
  const textColor = getContrastColor(ann.fill);

  return `<g><circle cx="${cx}" cy="${cy}" r="${radius}" fill="${ann.fill}"/><text x="${cx}" y="${cy}" text-anchor="middle" dominant-baseline="central" font-size="${fontSize}" font-weight="bold" font-family="system-ui, -apple-system, sans-serif" fill="${textColor}">${ann.displayValue}</text></g>`;
}

export {
  getDisplayValue,
  renumberAnnotations,
  getNextNumberValue,
} from './number-utils';
