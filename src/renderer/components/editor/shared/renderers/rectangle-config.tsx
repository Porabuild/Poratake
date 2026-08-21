import type { RectAnnotation } from '@/types/editor';
import type { ShapeRenderConfig, HandlePosition } from './types';
import { normalizeNegativeRect } from '@/renderer/utils/annotation-geometry';

export const rectangleConfig: ShapeRenderConfig<RectAnnotation> = {
  getGeometry(ann, offsetX, offsetY) {
    const x = ann.x + offsetX;
    const y = ann.y + offsetY;
    const bounds = normalizeNegativeRect(x, y, ann.width, ann.height);
    return {
      bounds,
      center: {
        x: bounds.x + bounds.width / 2,
        y: bounds.y + bounds.height / 2,
      },
    };
  },

  renderShape(ann, geometry) {
    const { x, y, width, height } = geometry.bounds;
    return (
      <rect
        x={x}
        y={y}
        width={width}
        height={height}
        fill={ann.fill ?? 'none'}
        stroke={ann.stroke}
        strokeWidth={ann.strokeWidth}
        rx="1"
      />
    );
  },

  renderSelectionShape(ann, geometry, _offsetX, _offsetY, stroke, strokeWidth) {
    const { x, y, width, height } = geometry.bounds;
    return (
      <rect
        x={x}
        y={y}
        width={width}
        height={height}
        fill="none"
        stroke={stroke}
        strokeWidth={ann.strokeWidth + strokeWidth}
        rx="1"
      />
    );
  },

  getHandlePositions(_ann, geometry): HandlePosition[] {
    const { x, y, width, height } = geometry.bounds;
    return [
      { pos: 'top-left', x, y, cursor: 'nwse-resize' },
      { pos: 'top-right', x: x + width, y, cursor: 'nesw-resize' },
      { pos: 'bottom-left', x, y: y + height, cursor: 'nesw-resize' },
      {
        pos: 'bottom-right',
        x: x + width,
        y: y + height,
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
    const x = (ann.x + offsetX) * scale;
    const y = (ann.y + offsetY) * scale;
    const w = ann.width * scale;
    const h = ann.height * scale;
    const rect = normalizeNegativeRect(x, y, w, h);
    return `<rect x="${rect.x}" y="${rect.y}" width="${rect.width}" height="${rect.height}" fill="${ann.fill ?? 'none'}" stroke="${ann.stroke}" stroke-width="${ann.strokeWidth * scale}" rx="1"/>`;
  },
};
