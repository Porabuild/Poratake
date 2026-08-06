import {
  useCallback,
  useRef,
  useState,
  forwardRef,
  useImperativeHandle,
} from 'react';
import type { Annotation } from '@/types/editor';
import {
  type ResizeHandle,
  type DragState,
  type ResizeState,
  HANDLE_SIZE,
} from './shared';
import {
  measureText,
  TEXT_BG_PADDING_X,
  TEXT_BG_PADDING_Y,
  TEXT_FONT_WEIGHT,
} from './text/text-utils';

import { renderPen, exportPen } from './pen';
import {
  renderRectangle,
  renderRectangleHandles,
  exportRectangle,
  renderCircle,
  renderCircleHandles,
  exportCircle,
  renderLine,
  renderLineHandles,
  exportLine,
} from './shapes';
import { renderArrow, renderArrowHandles, exportArrow } from './arrow';
import {
  renderText,
  renderTextHandles,
  exportText,
} from './text/text-renderer';
import { renderNumber, exportNumber } from './number';

export interface AnnotationsOverlayHandle {
  getSvgForExport: (scale: number) => string;
}

interface AnnotationsOverlayProps {
  annotations: Annotation[];
  currentAnnotation: Annotation | null;
  width: number;
  height: number;
  offsetX?: number;
  offsetY?: number;
  selectedAnnotationId: string | null;
  onSelect: (id: string | null) => void;
  onAnnotationUpdate?: (id: string, updates: Partial<Annotation>) => void;
  onDragEnd?: () => void;
  onTextDoubleClick?: (id: string) => void;
  editingTextId: string | null;
  zoom?: number;
}

const AnnotationsOverlay = forwardRef<
  AnnotationsOverlayHandle,
  AnnotationsOverlayProps
>(
  (
    {
      annotations,
      currentAnnotation,
      width,
      height,
      offsetX = 0,
      offsetY = 0,
      selectedAnnotationId,
      onSelect,
      onAnnotationUpdate,
      onDragEnd,
      editingTextId,
      onTextDoubleClick,
      zoom = 1,
    },
    ref
  ) => {
    const [dragState, setDragState] = useState<DragState | null>(null);
    const [resizeState, setResizeState] = useState<ResizeState | null>(null);
    const svgRef = useRef<SVGSVGElement>(null);

    const handleDoubleClick = useCallback(
      (e: React.MouseEvent, id: string) => {
        e.stopPropagation();
        onTextDoubleClick?.(id);
      },
      [onTextDoubleClick]
    );

    const handleMouseDown = useCallback(
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

    const handleMouseMove = useCallback(
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

        const annotation = annotations.find(
          a => a.id === dragState.annotationId
        );
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

    const handleMouseUp = useCallback(() => {
      if (dragState) {
        onDragEnd?.();
      }
      setDragState(null);
    }, [dragState, onDragEnd]);

    const handleMouseLeave = useCallback(() => {
      if (dragState || resizeState) {
        onDragEnd?.();
      }
      setDragState(null);
      setResizeState(null);
    }, [dragState, resizeState, onDragEnd]);

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
      [annotations, offsetX, offsetY, zoom]
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
      [resizeState, onAnnotationUpdate, zoom, annotations, offsetX, offsetY]
    );

    const handleResizeEnd = useCallback(() => {
      if (resizeState) {
        onDragEnd?.();
      }
      setResizeState(null);
    }, [resizeState, onDragEnd]);

    const handleCombinedMouseMove = useCallback(
      (e: React.MouseEvent) => {
        if (resizeState) {
          handleResizeMove(e);
        } else if (dragState) {
          handleMouseMove(e);
        }
      },
      [resizeState, dragState, handleResizeMove, handleMouseMove]
    );

    const handleCombinedMouseUp = useCallback(() => {
      handleMouseUp();
      handleResizeEnd();
    }, [handleMouseUp, handleResizeEnd]);

    const renderResizeHandles = (ann: Annotation) => {
      if (selectedAnnotationId !== ann.id) return null;

      const commonProps = {
        offsetX,
        offsetY,
        handleSize: HANDLE_SIZE,
        onResizeStart: handleResizeStart,
      };

      switch (ann.type) {
        case 'rectangle':
          return renderRectangleHandles({ annotation: ann, ...commonProps });
        case 'circle':
          return renderCircleHandles({ annotation: ann, ...commonProps });
        case 'line':
          return renderLineHandles({ annotation: ann, ...commonProps });
        case 'arrow':
          return renderArrowHandles({ annotation: ann, ...commonProps });
        case 'text':
          return renderTextHandles({ annotation: ann, ...commonProps });
        default:
          return null;
      }
    };

    const renderAnnotation = (ann: Annotation, isPreview = false) => {
      const isSelected = selectedAnnotationId === ann.id;

      const commonProps = {
        offsetX,
        offsetY,
        isSelected,
        isPreview,
        onMouseDown: handleMouseDown,
        onDoubleClick: handleDoubleClick,
      };

      switch (ann.type) {
        case 'pen':
          return renderPen({ annotation: ann, ...commonProps });
        case 'rectangle':
          return renderRectangle({ annotation: ann, ...commonProps });
        case 'circle':
          return renderCircle({ annotation: ann, ...commonProps });
        case 'line':
          return renderLine({ annotation: ann, ...commonProps });
        case 'arrow':
          return renderArrow({ annotation: ann, ...commonProps });
        case 'text':
          return renderText({
            annotation: ann,
            ...commonProps,
            editingTextId,
          });
        case 'number':
          return renderNumber({ annotation: ann, ...commonProps });
        default:
          return null;
      }
    };

    const getSvgForExport = useCallback(
      (scale: number): string => {
        const scaledWidth = width * scale;
        const scaledHeight = height * scale;

        const exportProps = { offsetX, offsetY, scale };

        const renderAnnotationForExport = (ann: Annotation): string => {
          switch (ann.type) {
            case 'pen':
              return exportPen({ annotation: ann, ...exportProps });
            case 'rectangle':
              return exportRectangle({ annotation: ann, ...exportProps });
            case 'circle':
              return exportCircle({ annotation: ann, ...exportProps });
            case 'line':
              return exportLine({ annotation: ann, ...exportProps });
            case 'arrow':
              return exportArrow({ annotation: ann, ...exportProps });
            case 'text':
              return exportText({ annotation: ann, ...exportProps });
            case 'number':
              return exportNumber({ annotation: ann, ...exportProps });
            default:
              return '';
          }
        };

        const annotationsContent = annotations
          .map(renderAnnotationForExport)
          .join('');

        return `<svg xmlns="http://www.w3.org/2000/svg" width="${scaledWidth}" height="${scaledHeight}">${annotationsContent}</svg>`;
      },
      [annotations, width, height, offsetX, offsetY]
    );

    useImperativeHandle(ref, () => ({
      getSvgForExport,
    }));

    return (
      <svg
        ref={svgRef}
        width={width}
        height={height}
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          pointerEvents: dragState || resizeState ? 'auto' : 'none',
          overflow: 'visible',
        }}
        onMouseMove={handleCombinedMouseMove}
        onMouseUp={handleCombinedMouseUp}
        onMouseLeave={handleMouseLeave}
      >
        <g>
          {annotations.map(ann => renderAnnotation(ann))}
          {currentAnnotation && renderAnnotation(currentAnnotation, true)}
          {annotations.map(ann => (
            <g key={`handles-${ann.id}`}>{renderResizeHandles(ann)}</g>
          ))}
        </g>
      </svg>
    );
  }
);

AnnotationsOverlay.displayName = 'AnnotationsOverlay';

export default AnnotationsOverlay;
