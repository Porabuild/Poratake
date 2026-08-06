import type { LineAnnotation } from '@/types/editor';
import type { ShapeRenderConfig, HandlePosition } from './types';

export const lineConfig: ShapeRenderConfig<LineAnnotation> = {
  getGeometry(ann, offsetX, offsetY) {
    const [x1, y1, x2, y2] = ann.points;
    const minX = Math.min(x1, x2) + offsetX;
    const minY = Math.min(y1, y2) + offsetY;
    const maxX = Math.max(x1, x2) + offsetX;
    const maxY = Math.max(y1, y2) + offsetY;
    return {
      bounds: {
        x: minX,
        y: minY,
        width: maxX - minX,
        height: maxY - minY,
      },
      center: {
        x: (x1 + x2) / 2 + offsetX,
        y: (y1 + y2) / 2 + offsetY,
      },
    };
  },

  renderShape(ann, _geometry, offsetX, offsetY) {
    const [x1, y1, x2, y2] = ann.points;
    return (
      <line
        x1={x1 + offsetX}
        y1={y1 + offsetY}
        x2={x2 + offsetX}
        y2={y2 + offsetY}
        stroke={ann.stroke}
        strokeWidth={ann.strokeWidth}
        strokeLinecap="round"
      />
    );
  },

  renderSelectionShape(ann, _geometry, offsetX, offsetY, stroke, strokeWidth) {
    const [x1, y1, x2, y2] = ann.points;
    return (
      <line
        x1={x1 + offsetX}
        y1={y1 + offsetY}
        x2={x2 + offsetX}
        y2={y2 + offsetY}
        stroke={stroke}
        strokeWidth={ann.strokeWidth + strokeWidth}
        strokeLinecap="round"
      />
    );
  },

  getHandlePositions(ann, _geometry, offsetX, offsetY): HandlePosition[] {
    const [x1, y1, x2, y2] = ann.points;
    return [
      { pos: 'start', x: x1 + offsetX, y: y1 + offsetY, cursor: 'move' },
      { pos: 'end', x: x2 + offsetX, y: y2 + offsetY, cursor: 'move' },
    ];
  },

  renderHandle(_ann, handle, handleSize, handleStyle) {
    return (
      <circle
        cx={handle.x}
        cy={handle.y}
        r={handleSize / 2}
        fill={handleStyle.fill}
        stroke={handleStyle.stroke}
        strokeWidth={handleStyle.strokeWidth}
        style={{ pointerEvents: handleStyle.pointerEvents }}
      />
    );
  },

  exportShape(ann, offsetX, offsetY, scale) {
    const [x1, y1, x2, y2] = ann.points;
    return `<line x1="${(x1 + offsetX) * scale}" y1="${(y1 + offsetY) * scale}" x2="${(x2 + offsetX) * scale}" y2="${(y2 + offsetY) * scale}" stroke="${ann.stroke}" stroke-width="${ann.strokeWidth * scale}" stroke-linecap="round"/>`;
  },
};
