import type { CircleAnnotation } from '@/types/editor';
import type { ShapeRenderConfig, HandlePosition } from './types';

const DIAGONAL_FACTOR = 0.707;

export const circleConfig: ShapeRenderConfig<CircleAnnotation> = {
  getGeometry(ann, offsetX, offsetY) {
    const cx = ann.x + offsetX;
    const cy = ann.y + offsetY;
    return {
      bounds: {
        x: cx - ann.radius,
        y: cy - ann.radius,
        width: ann.radius * 2,
        height: ann.radius * 2,
      },
      center: { x: cx, y: cy },
    };
  },

  renderShape(ann, geometry) {
    return (
      <circle
        cx={geometry.center.x}
        cy={geometry.center.y}
        r={ann.radius}
        fill={ann.fill ?? 'none'}
        stroke={ann.stroke}
        strokeWidth={ann.strokeWidth}
      />
    );
  },

  renderSelectionShape(ann, geometry, _offsetX, _offsetY, stroke, strokeWidth) {
    return (
      <circle
        cx={geometry.center.x}
        cy={geometry.center.y}
        r={ann.radius}
        fill="none"
        stroke={stroke}
        strokeWidth={ann.strokeWidth + strokeWidth}
      />
    );
  },

  getHandlePositions(ann, geometry): HandlePosition[] {
    const { x: cx, y: cy } = geometry.center;
    const offset = ann.radius * DIAGONAL_FACTOR;
    return [
      {
        pos: 'top-right',
        x: cx + offset,
        y: cy - offset,
        cursor: 'nesw-resize',
      },
      {
        pos: 'bottom-right',
        x: cx + offset,
        y: cy + offset,
        cursor: 'nwse-resize',
      },
      {
        pos: 'bottom-left',
        x: cx - offset,
        y: cy + offset,
        cursor: 'nesw-resize',
      },
      {
        pos: 'top-left',
        x: cx - offset,
        y: cy - offset,
        cursor: 'nwse-resize',
      },
    ];
  },

  renderHandle(_ann, handle, handleSize, handleStyle) {
    return (
      <rect
        x={handle.x - handleSize / 2}
        y={handle.y - handleSize / 2}
        width={handleSize}
        height={handleSize}
        rx={2}
        fill={handleStyle.fill}
        stroke={handleStyle.stroke}
        strokeWidth={handleStyle.strokeWidth}
        style={{ pointerEvents: handleStyle.pointerEvents }}
      />
    );
  },

  exportShape(ann, offsetX, offsetY, scale) {
    const cx = (ann.x + offsetX) * scale;
    const cy = (ann.y + offsetY) * scale;
    const r = ann.radius * scale;
    return `<circle cx="${cx}" cy="${cy}" r="${r}" fill="${ann.fill ?? 'none'}" stroke="${ann.stroke}" stroke-width="${ann.strokeWidth * scale}"/>`;
  },
};
