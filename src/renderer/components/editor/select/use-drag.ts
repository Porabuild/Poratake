import { useState, useCallback, useRef } from 'react';
import type { Annotation } from '@/types/editor';
import type { DragState } from '../shared';

interface UseDragOptions {
  annotations: Annotation[];
  zoom: number;
  onSelect: (id: string | null) => void;
  onAnnotationUpdate?: (id: string, updates: Partial<Annotation>) => void;
  onDragEnd?: () => void;
}

export function useDrag({
  annotations,
  zoom,
  onSelect,
  onAnnotationUpdate,
  onDragEnd,
}: UseDragOptions) {
  const [dragState, setDragState] = useState<DragState | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  const handleDragStart = useCallback(
    (e: React.MouseEvent, id: string) => {
      e.stopPropagation();
      e.preventDefault();

      const annotation = annotations.find(a => a.id === id);
      if (!annotation) return;

      onSelect(id);

      const svg = svgRef.current;
      if (!svg) return;

      const rect = svg.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      setDragState({
        annotationId: id,
        lastX: x,
        lastY: y,
      });
    },
    [annotations, onSelect]
  );

  const handleDragMove = useCallback(
    (e: React.MouseEvent) => {
      if (!dragState || !onAnnotationUpdate) return;

      const svg = svgRef.current;
      if (!svg) return;

      const rect = svg.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      const dx = (x - dragState.lastX) / zoom;
      const dy = (y - dragState.lastY) / zoom;

      setDragState(prev => (prev ? { ...prev, lastX: x, lastY: y } : null));

      const annotation = annotations.find(a => a.id === dragState.annotationId);
      if (!annotation) return;

      switch (annotation.type) {
        case 'pen':
        case 'line':
        case 'arrow': {
          const newPoints = annotation.points.map((p, i) =>
            i % 2 === 0 ? p + dx : p + dy
          );
          onAnnotationUpdate(dragState.annotationId, { points: newPoints });
          break;
        }
        case 'rectangle':
        case 'circle':
        case 'text':
        case 'number': {
          onAnnotationUpdate(dragState.annotationId, {
            x: annotation.x + dx,
            y: annotation.y + dy,
          });
          break;
        }
      }
    },
    [dragState, onAnnotationUpdate, zoom, annotations]
  );

  const handleDragEnd = useCallback(() => {
    if (dragState) {
      onDragEnd?.();
    }
    setDragState(null);
  }, [dragState, onDragEnd]);

  return {
    dragState,
    svgRef,
    handleDragStart,
    handleDragMove,
    handleDragEnd,
  };
}
