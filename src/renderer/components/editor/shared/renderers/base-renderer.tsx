import type { JSX } from 'react';
import type { Annotation } from '@/types/editor';
import { SELECTION_STROKE, SELECTION_STROKE_WIDTH } from '../colors';
import { getHandleStyle } from '../types';
import type {
  ShapeRenderConfig,
  ShapeRenderProps,
  ShapeHandleProps,
  ShapeExportProps,
} from './types';

export function createShapeRenderer<T extends Annotation>(
  config: ShapeRenderConfig<T>
) {
  function render({
    annotation: ann,
    offsetX,
    offsetY,
    isSelected,
    isPreview,
    onMouseDown,
  }: ShapeRenderProps<T>): JSX.Element {
    const key = isPreview ? `preview-${ann.id}` : ann.id;
    const geometry = config.getGeometry(ann, offsetX, offsetY);

    return (
      <g
        key={key}
        style={{
          cursor: isPreview ? 'default' : 'move',
          pointerEvents: isPreview ? 'none' : 'auto',
        }}
        onMouseDown={isPreview ? undefined : e => onMouseDown?.(e, ann.id)}
      >
        {isSelected &&
          config.renderSelectionShape(
            ann,
            geometry,
            offsetX,
            offsetY,
            SELECTION_STROKE,
            SELECTION_STROKE_WIDTH
          )}
        {config.renderShape(ann, geometry, offsetX, offsetY)}
      </g>
    );
  }

  function renderHandles({
    annotation: ann,
    offsetX,
    offsetY,
    handleSize,
    onResizeStart,
  }: ShapeHandleProps<T>): JSX.Element {
    const geometry = config.getGeometry(ann, offsetX, offsetY);
    const handles = config.getHandlePositions(ann, geometry, offsetX, offsetY);

    const handleStyle = {
      ...getHandleStyle(),
      pointerEvents: 'auto' as const,
    };

    return (
      <>
        {handles.map(handle => (
          <g
            key={`handle-${ann.id}-${handle.pos}`}
            style={{ cursor: handle.cursor }}
            onMouseDown={e => onResizeStart(e, ann.id, handle.pos)}
          >
            {config.renderHandle(ann, handle, handleSize, handleStyle)}
          </g>
        ))}
      </>
    );
  }

  function exportShape({
    annotation: ann,
    offsetX,
    offsetY,
    scale,
  }: ShapeExportProps<T>): string {
    return config.exportShape(ann, offsetX, offsetY, scale);
  }

  return { render, renderHandles, exportShape };
}
