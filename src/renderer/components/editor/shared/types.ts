import type { Annotation } from '@/types/editor';

export type ResizeHandle =
  | 'top-left'
  | 'top-right'
  | 'bottom-left'
  | 'bottom-right'
  | 'start'
  | 'end'
  | 'bend'
  | 'rotate';

export interface DragState {
  annotationId: string;
  lastX: number;
  lastY: number;
}

export interface ResizeState {
  annotationId: string;
  handle: ResizeHandle;
  startX: number;
  startY: number;
  lastX: number;
  lastY: number;
  anchorX?: number;
  anchorY?: number;
  initialDragX?: number;
  initialDragY?: number;
  initialFontSize?: number;
  initialWidth?: number;
  initialHeight?: number;
  centerX?: number;
  centerY?: number;
  initialRotation?: number;
  startAngle?: number;
}

export interface AnnotationRenderProps {
  annotation: Annotation;
  offsetX: number;
  offsetY: number;
  isSelected: boolean;
  isPreview: boolean;
  onMouseDown?: (e: React.MouseEvent, id: string) => void;
  onDoubleClick?: (e: React.MouseEvent, id: string) => void;
}

export interface ResizeHandleRenderProps {
  annotation: Annotation;
  offsetX: number;
  offsetY: number;
  handleSize: number;
  onResizeStart: (
    e: React.MouseEvent,
    id: string,
    handle: ResizeHandle
  ) => void;
}

export interface ExportRenderProps {
  annotation: Annotation;
  offsetX: number;
  offsetY: number;
  scale: number;
}

export const HANDLE_SIZE = 12;

export const getHandleStyle = () => ({
  fill: 'white',
  stroke: '#007AFF',
  strokeWidth: 2,
  cursor: 'pointer',
});
