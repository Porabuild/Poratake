import type { PenAnnotation } from '@/types/editor';
import { getStroke } from 'perfect-freehand';
import {
  type AnnotationRenderProps,
  type ExportRenderProps,
  SELECTION_STROKE,
} from './types';

export const pointsToCoordinates = (points: number[]): [number, number][] => {
  const coords: [number, number][] = [];
  for (let i = 0; i < points.length; i += 2) {
    coords.push([points[i], points[i + 1]]);
  }
  return coords;
};

export const getSvgPathFromStroke = (stroke: number[][]): string => {
  if (!stroke.length) return '';

  const d = stroke.reduce(
    (acc, [x0, y0], i, arr) => {
      const [x1, y1] = arr[(i + 1) % arr.length];
      acc.push(x0, y0, (x0 + x1) / 2, (y0 + y1) / 2);
      return acc;
    },
    ['M', ...stroke[0], 'Q']
  );

  d.push('Z');
  return d.join(' ');
};

const freehandOptions = {
  thinning: 0.5,
  smoothing: 0.6,
  streamline: 0.5,
  simulatePressure: true,
};

interface PenRenderProps extends Omit<AnnotationRenderProps, 'annotation'> {
  annotation: PenAnnotation;
}

export function renderPen({
  annotation: ann,
  offsetX,
  offsetY,
  isSelected,
  isPreview,
  onMouseDown,
}: PenRenderProps): JSX.Element {
  const key = isPreview ? `preview-${ann.id}` : ann.id;

  const coords = pointsToCoordinates(ann.points).map(
    ([x, y]) => [x + offsetX, y + offsetY] as [number, number]
  );

  const outlinePoints = getStroke(coords, {
    size: ann.strokeWidth * 2,
    ...freehandOptions,
  });

  const pathData = getSvgPathFromStroke(outlinePoints);

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
        <path
          d={pathData}
          fill={SELECTION_STROKE}
          stroke={SELECTION_STROKE}
          strokeWidth={2}
        />
      )}
      <path d={pathData} fill={ann.stroke} />
    </g>
  );
}

interface PenExportProps extends Omit<ExportRenderProps, 'annotation'> {
  annotation: PenAnnotation;
}

export function exportPen({
  annotation: ann,
  offsetX,
  offsetY,
  scale,
}: PenExportProps): string {
  const coords = pointsToCoordinates(ann.points).map(
    ([x, y]) =>
      [(x + offsetX) * scale, (y + offsetY) * scale] as [number, number]
  );

  const outlinePoints = getStroke(coords, {
    size: ann.strokeWidth * 2 * scale,
    ...freehandOptions,
  });

  const pathData = getSvgPathFromStroke(outlinePoints);
  return `<path d="${pathData}" fill="${ann.stroke}"/>`;
}
