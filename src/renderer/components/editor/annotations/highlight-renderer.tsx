import type { JSX } from 'react';
import type { HighlightAnnotation } from '@/types/editor';
import {
  type AnnotationRenderProps,
  type ExportRenderProps,
  SELECTION_STROKE,
} from './types';

const pointsToCoordinates = (points: number[]): [number, number][] => {
  const coords: [number, number][] = [];
  for (let i = 0; i < points.length; i += 2) {
    coords.push([points[i], points[i + 1]]);
  }
  return coords;
};

const getHighlighterPath = (
  points: [number, number][],
  strokeWidth: number
): string => {
  if (points.length < 2) return '';

  const halfWidth = strokeWidth / 2;

  const upperEdge: [number, number][] = [];
  const lowerEdge: [number, number][] = [];

  for (let i = 0; i < points.length; i++) {
    const [x, y] = points[i];

    let dx = 0;
    let dy = 1;

    if (i < points.length - 1) {
      const [nx, ny] = points[i + 1];
      const len = Math.sqrt((nx - x) ** 2 + (ny - y) ** 2);
      if (len > 0) {
        dx = -(ny - y) / len;
        dy = (nx - x) / len;
      }
    } else if (i > 0) {
      const [px, py] = points[i - 1];
      const len = Math.sqrt((x - px) ** 2 + (y - py) ** 2);
      if (len > 0) {
        dx = -(y - py) / len;
        dy = (x - px) / len;
      }
    }

    upperEdge.push([x + dx * halfWidth, y + dy * halfWidth]);
    lowerEdge.push([x - dx * halfWidth, y - dy * halfWidth]);
  }

  const pathParts: string[] = [];
  pathParts.push(`M ${upperEdge[0][0]} ${upperEdge[0][1]}`);

  for (let i = 1; i < upperEdge.length; i++) {
    pathParts.push(`L ${upperEdge[i][0]} ${upperEdge[i][1]}`);
  }

  for (let i = lowerEdge.length - 1; i >= 0; i--) {
    pathParts.push(`L ${lowerEdge[i][0]} ${lowerEdge[i][1]}`);
  }

  pathParts.push('Z');
  return pathParts.join(' ');
};

interface HighlightRenderProps extends Omit<
  AnnotationRenderProps,
  'annotation'
> {
  annotation: HighlightAnnotation;
}

export function renderHighlight({
  annotation: ann,
  offsetX,
  offsetY,
  isSelected,
  isPreview,
  onMouseDown,
}: HighlightRenderProps): JSX.Element {
  const key = isPreview ? `preview-${ann.id}` : ann.id;

  const coords = pointsToCoordinates(ann.points).map(
    ([x, y]) => [x + offsetX, y + offsetY] as [number, number]
  );

  const pathData = getHighlighterPath(coords, ann.strokeWidth);

  if (!pathData) {
    return <g key={key} />;
  }

  return (
    <g
      key={key}
      style={{
        cursor: isPreview ? 'default' : 'move',
        pointerEvents: isPreview ? 'none' : 'auto',
        mixBlendMode: 'multiply',
      }}
      onMouseDown={isPreview ? undefined : e => onMouseDown?.(e, ann.id)}
    >
      {isSelected && (
        <path
          d={pathData}
          fill="none"
          stroke={SELECTION_STROKE}
          strokeWidth={3}
        />
      )}
      <path d={pathData} fill={ann.fill} opacity={ann.opacity} />
    </g>
  );
}

interface HighlightExportProps extends Omit<ExportRenderProps, 'annotation'> {
  annotation: HighlightAnnotation;
}

export function exportHighlight({
  annotation: ann,
  offsetX,
  offsetY,
  scale,
}: HighlightExportProps): string {
  const coords = pointsToCoordinates(ann.points).map(
    ([x, y]) =>
      [(x + offsetX) * scale, (y + offsetY) * scale] as [number, number]
  );

  const pathData = getHighlighterPath(coords, ann.strokeWidth * scale);

  if (!pathData) return '';

  return `<path d="${pathData}" fill="${ann.fill}" opacity="${ann.opacity}" style="mix-blend-mode: multiply"/>`;
}
