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

export const SELECTION_STROKE =
  'color-mix(in srgb, var(--primary) 80%, transparent)';
export const SELECTION_STROKE_WIDTH = 6;
