import { useState, useCallback } from 'react';
import type { Annotation } from '@/types/editor';
import type { ResizeState, ResizeHandle } from '../shared';
import {
  measureText,
  TEXT_BG_PADDING_X,
  TEXT_BG_PADDING_Y,
  TEXT_FONT_WEIGHT,
} from '../text/text-utils';

interface UseResizeOptions {
  annotations: Annotation[];
  zoom: number;
  offsetX: number;
  offsetY: number;
  onAnnotationUpdate?: (id: string, updates: Partial<Annotation>) => void;
  onResizeEnd?: () => void;
  svgRef: React.RefObject<SVGSVGElement | null>;
}

export function useResize({
  annotations,
  zoom,
  offsetX,
  offsetY,
  onAnnotationUpdate,
  onResizeEnd,
  svgRef,
}: UseResizeOptions) {
  const [resizeState, setResizeState] = useState<ResizeState | null>(null);

  const handleResizeStart = useCallback(
    (e: React.MouseEvent, id: string, handle: ResizeHandle) => {
      e.stopPropagation();
      e.preventDefault();

      const annotation = annotations.find(a => a.id === id);
      if (!annotation) return;

      const svg = svgRef.current;
      if (!svg) return;

      const rect = svg.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      let anchorX: number | undefined;
      let anchorY: number | undefined;
      let initialDragX: number | undefined;
      let initialDragY: number | undefined;
      let initialFontSize: number | undefined;
      let initialWidth: number | undefined;
      let initialHeight: number | undefined;
      let centerX: number | undefined;
      let centerY: number | undefined;
      let initialRotation: number | undefined;
      let startAngle: number | undefined;

      if (annotation.type === 'rectangle') {
        const w = annotation.width;
        const h = annotation.height;
        const visualLeft = w < 0 ? annotation.x + w : annotation.x;
        const visualTop = h < 0 ? annotation.y + h : annotation.y;
        const visualRight = w < 0 ? annotation.x : annotation.x + w;
        const visualBottom = h < 0 ? annotation.y : annotation.y + h;

        if (handle === 'top-left') {
          anchorX = visualRight;
          anchorY = visualBottom;
          initialDragX = visualLeft;
          initialDragY = visualTop;
        } else if (handle === 'top-right') {
          anchorX = visualLeft;
          anchorY = visualBottom;
          initialDragX = visualRight;
          initialDragY = visualTop;
        } else if (handle === 'bottom-left') {
          anchorX = visualRight;
          anchorY = visualTop;
          initialDragX = visualLeft;
          initialDragY = visualBottom;
        } else if (handle === 'bottom-right') {
          anchorX = visualLeft;
          anchorY = visualTop;
          initialDragX = visualRight;
          initialDragY = visualBottom;
        }
      } else if (annotation.type === 'text') {
        const fontFamily = annotation.fontFamily || 'Arial, sans-serif';
        const measured = measureText(
          annotation.text,
          annotation.fontSize,
          fontFamily,
          TEXT_FONT_WEIGHT
        );
        const hasBackground = !!annotation.backgroundColor;
        const bgPadding = hasBackground
          ? annotation.backgroundPadding || {
              x: TEXT_BG_PADDING_X,
              y: TEXT_BG_PADDING_Y,
            }
          : { x: 0, y: 0 };

        initialFontSize = annotation.fontSize;
        initialWidth = measured.width + bgPadding.x * 2;
        initialHeight = measured.height + bgPadding.y * 2;

        centerX = annotation.x + initialWidth / 2;
        centerY = annotation.y + initialHeight / 2;
        initialRotation = annotation.rotation || 0;

        if (handle === 'rotate') {
          const mouseX = (x - offsetX * zoom) / zoom;
          const mouseY = (y - offsetY * zoom) / zoom;
          startAngle = Math.atan2(mouseY - centerY, mouseX - centerX);
        }
      }

      setResizeState({
        annotationId: id,
        handle,
        startX: x,
        startY: y,
        lastX: x,
        lastY: y,
        anchorX,
        anchorY,
        initialDragX,
        initialDragY,
        initialFontSize,
        initialWidth,
        initialHeight,
        centerX,
        centerY,
        initialRotation,
        startAngle,
      });
    },
    [annotations, offsetX, offsetY, zoom, svgRef]
  );

  const handleResizeMove = useCallback(
    (e: React.MouseEvent) => {
      if (!resizeState || !onAnnotationUpdate) return;

      const svg = svgRef.current;
      if (!svg) return;

      const rect = svg.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      const dx = (x - resizeState.lastX) / zoom;
      const dy = (y - resizeState.lastY) / zoom;

      setResizeState(prev => (prev ? { ...prev, lastX: x, lastY: y } : null));

      const annotation = annotations.find(
        a => a.id === resizeState.annotationId
      );
      if (!annotation) return;

      const handle = resizeState.handle;

      switch (annotation.type) {
        case 'rectangle': {
          const {
            anchorX,
            anchorY,
            initialDragX,
            initialDragY,
            startX,
            startY,
          } = resizeState;
          if (
            anchorX === undefined ||
            anchorY === undefined ||
            initialDragX === undefined ||
            initialDragY === undefined
          )
            break;

          const totalDx = (x - startX) / zoom;
          const totalDy = (y - startY) / zoom;

          const newDragX = initialDragX + totalDx;
          const newDragY = initialDragY + totalDy;

          onAnnotationUpdate(resizeState.annotationId, {
            x: anchorX,
            y: anchorY,
            width: newDragX - anchorX,
            height: newDragY - anchorY,
          });
          break;
        }

        case 'circle': {
          let radiusDelta = 0;

          if (handle === 'bottom-right') {
            radiusDelta = (dx + dy) / 2;
          } else if (handle === 'top-left') {
            radiusDelta = (-dx - dy) / 2;
          } else if (handle === 'top-right') {
            radiusDelta = (dx - dy) / 2;
          } else if (handle === 'bottom-left') {
            radiusDelta = (-dx + dy) / 2;
          }

          const newRadius = Math.max(5, annotation.radius + radiusDelta);
          onAnnotationUpdate(resizeState.annotationId, {
            radius: newRadius,
          });
          break;
        }

        case 'line': {
          const newPoints = [...annotation.points];
          if (handle === 'start') {
            newPoints[0] = annotation.points[0] + dx;
            newPoints[1] = annotation.points[1] + dy;
          } else if (handle === 'end') {
            newPoints[2] = annotation.points[2] + dx;
            newPoints[3] = annotation.points[3] + dy;
          }
          onAnnotationUpdate(resizeState.annotationId, { points: newPoints });
          break;
        }

        case 'arrow': {
          if (handle === 'bend') {
            const currentBend = annotation.bendOffset || { x: 0, y: 0 };
            onAnnotationUpdate(resizeState.annotationId, {
              bendOffset: {
                x: currentBend.x + dx,
                y: currentBend.y + dy,
              },
            });
          } else {
            const newPoints = [...annotation.points];
            if (handle === 'start') {
              newPoints[0] = annotation.points[0] + dx;
              newPoints[1] = annotation.points[1] + dy;
            } else if (handle === 'end') {
              newPoints[2] = annotation.points[2] + dx;
              newPoints[3] = annotation.points[3] + dy;
            }
            onAnnotationUpdate(resizeState.annotationId, {
              points: newPoints,
            });
          }
          break;
        }

        case 'text': {
          const {
            initialFontSize,
            initialWidth,
            initialHeight,
            centerX: cX,
            centerY: cY,
            initialRotation,
            startAngle,
            startX: sX,
            startY: sY,
          } = resizeState;

          if (handle === 'rotate') {
            if (
              cX === undefined ||
              cY === undefined ||
              initialRotation === undefined ||
              startAngle === undefined
            )
              break;

            const mouseX = (x - offsetX * zoom) / zoom;
            const mouseY = (y - offsetY * zoom) / zoom;
            const currentAngle = Math.atan2(mouseY - cY, mouseX - cX);

            const angleDelta = ((currentAngle - startAngle) * 180) / Math.PI;
            const newRotation = initialRotation + angleDelta;

            onAnnotationUpdate(resizeState.annotationId, {
              rotation: newRotation,
            });
          } else if (handle === 'bottom-right') {
            if (
              initialFontSize === undefined ||
              initialWidth === undefined ||
              initialHeight === undefined
            )
              break;

            const totalDx = (x - sX) / zoom;
            const totalDy = (y - sY) / zoom;

            const newWidth = initialWidth + totalDx;
            const newHeight = initialHeight + totalDy;

            const scaleX = newWidth / initialWidth;
            const scaleY = newHeight / initialHeight;
            const scale = Math.max(0.1, Math.max(scaleX, scaleY));

            const newFontSize = Math.max(
              8,
              Math.round(initialFontSize * scale)
            );

            onAnnotationUpdate(resizeState.annotationId, {
              fontSize: newFontSize,
            });
          }
          break;
        }
      }
    },
    [
      resizeState,
      onAnnotationUpdate,
      zoom,
      annotations,
      offsetX,
      offsetY,
      svgRef,
    ]
  );

  const handleResizeEnd = useCallback(() => {
    if (resizeState) {
      onResizeEnd?.();
    }
    setResizeState(null);
  }, [resizeState, onResizeEnd]);

  return {
    resizeState,
    handleResizeStart,
    handleResizeMove,
    handleResizeEnd,
  };
}
