import type { Annotation } from '@/types/editor';
import type { ResizeHandle } from '../types';

export interface ShapeGeometry {
  bounds: { x: number; y: number; width: number; height: number };
  center: { x: number; y: number };
}

export interface HandlePosition {
  pos: ResizeHandle;
  x: number;
  y: number;
  cursor: string;
}

export interface HandleStyle {
  fill: string;
  stroke: string;
  strokeWidth: number;
  cursor: string;
  pointerEvents: 'auto';
}

export interface ShapeRenderConfig<T extends Annotation> {
  getGeometry: (ann: T, offsetX: number, offsetY: number) => ShapeGeometry;
  renderShape: (
    ann: T,
    geometry: ShapeGeometry,
    offsetX: number,
    offsetY: number
  ) => JSX.Element;
  renderSelectionShape: (
    ann: T,
    geometry: ShapeGeometry,
    offsetX: number,
    offsetY: number,
    selectionStroke: string,
    selectionStrokeWidth: number
  ) => JSX.Element;
  getHandlePositions: (
    ann: T,
    geometry: ShapeGeometry,
    offsetX: number,
    offsetY: number
  ) => HandlePosition[];
  renderHandle: (
    ann: T,
    handle: HandlePosition,
    handleSize: number,
    handleStyle: HandleStyle
  ) => JSX.Element;
  exportShape: (
    ann: T,
    offsetX: number,
    offsetY: number,
    scale: number
  ) => string;
}

export interface ShapeRenderProps<T extends Annotation> {
  annotation: T;
  offsetX: number;
  offsetY: number;
  isSelected: boolean;
  isPreview: boolean;
  onMouseDown?: (e: React.MouseEvent, id: string) => void;
}

export interface ShapeHandleProps<T extends Annotation> {
  annotation: T;
  offsetX: number;
  offsetY: number;
  handleSize: number;
  onResizeStart: (
    e: React.MouseEvent,
    id: string,
    handle: ResizeHandle
  ) => void;
}

export interface ShapeExportProps<T extends Annotation> {
  annotation: T;
  offsetX: number;
  offsetY: number;
  scale: number;
}
