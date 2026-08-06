import {
  useCallback,
  useRef,
  useState,
  useEffect,
  forwardRef,
  useImperativeHandle,
} from 'react';
import type { Annotation, ToolType } from '@/types/editor';
import {
  getBoundingBox,
  boxesIntersect,
  type BoundingBox,
} from './utils/boundingBox';
import {
  measureText,
  TEXT_BG_PADDING_X,
  TEXT_BG_PADDING_Y,
  TEXT_FONT_WEIGHT,
} from './utils/textUtils';

import {
  type ResizeHandle,
  renderPen,
  exportPen,
  renderHighlight,
  renderRectangle,
  renderRectangleHandles,
  exportRectangle,
  renderCircle,
  renderCircleHandles,
  exportCircle,
  renderLine,
  renderLineHandles,
  exportLine,
  renderArrow,
  renderArrowHandles,
  exportArrow,
  renderText,
  renderTextHandles,
  exportText,
  renderNumber,
  exportNumber,
  renderRedact,
  renderRedactHandles,
  exportRedact,
} from './annotations';

export interface SvgAnnotationsOverlayHandle {
  getSvgForExport: (scale: number) => string;
}

interface SvgAnnotationsOverlayProps {
  annotations: Annotation[];
  currentAnnotation: Annotation | null;
  width: number;
  height: number;
  offsetX?: number;
  offsetY?: number;
  selectedAnnotationIds: string[];
  onSelect: (id: string | null, addToSelection?: boolean) => void;
  onSelectMultiple?: (ids: string[]) => void;
  onAnnotationUpdate?: (id: string, updates: Partial<Annotation>) => void;
  onAnnotationsUpdateMultiple?: (
    updates: Array<{ id: string; updates: Partial<Annotation> }>
  ) => void;
  onDragEnd?: () => void;
  onTextDoubleClick?: (id: string) => void;
  editingTextId: string | null;
  zoom?: number;
  activeTool?: ToolType;
}

interface MarqueeState {
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
}

interface DragState {
  annotationId: string;
  lastX: number;
  lastY: number;
}

interface ResizeState {
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

const HANDLE_SIZE = 12;

const SvgAnnotationsOverlay = forwardRef<
  SvgAnnotationsOverlayHandle,
  SvgAnnotationsOverlayProps
>(
  (
    {
      annotations,
      currentAnnotation,
      width,
      height,
      offsetX = 0,
      offsetY = 0,
      selectedAnnotationIds,
      onSelect,
      onSelectMultiple,
      onAnnotationUpdate,
      onAnnotationsUpdateMultiple,
      onDragEnd,
      editingTextId,
      onTextDoubleClick,
      zoom = 1,
      activeTool = 'select',
    },
    ref
  ) => {
    const [dragState, setDragState] = useState<DragState | null>(null);
    const [resizeState, setResizeState] = useState<ResizeState | null>(null);
    const [marqueeState, setMarqueeState] = useState<MarqueeState | null>(null);
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

        const isShiftKey = e.shiftKey;
        const isAlreadySelected = selectedAnnotationIds.includes(id);

        if (isShiftKey) {
          onSelect(id, true);
        } else if (!isAlreadySelected) {
          onSelect(id, false);
        }

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
      [annotations, onSelect, selectedAnnotationIds]
    );

    const getAnnotationMoveUpdates = useCallback(
      (
        annotation: Annotation,
        dx: number,
        dy: number
      ): Partial<Annotation> | null => {
        switch (annotation.type) {
          case 'pen':
          case 'highlight':
          case 'line':
          case 'arrow': {
            const newPoints = annotation.points.map((p, i) =>
              i % 2 === 0 ? p + dx : p + dy
            );
            return { points: newPoints };
          }
          case 'rectangle':
          case 'circle':
          case 'text':
          case 'number':
          case 'redact': {
            return {
              x: annotation.x + dx,
              y: annotation.y + dy,
            };
          }
          default:
            return null;
        }
      },
      []
    );

    useEffect(() => {
      const isInteracting = dragState || resizeState || marqueeState;
      if (!isInteracting) return;

      const svg = svgRef.current;
      if (!svg) return;

      const handleWindowMouseMove = (e: MouseEvent) => {
        const rect = svg.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;

        if (resizeState && onAnnotationUpdate) {
          const dx = (x - resizeState.lastX) / zoom;
          const dy = (y - resizeState.lastY) / zoom;

          setResizeState(prev =>
            prev ? { ...prev, lastX: x, lastY: y } : null
          );

          const annotation = annotations.find(
            a => a.id === resizeState.annotationId
          );
          if (!annotation) return;

          const handle = resizeState.handle;

          switch (annotation.type) {
            case 'rectangle':
            case 'redact': {
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
              onAnnotationUpdate(resizeState.annotationId, {
                points: newPoints,
              });
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

                const angleDelta =
                  ((currentAngle - startAngle) * 180) / Math.PI;
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
        } else if (dragState) {
          const dx = (x - dragState.lastX) / zoom;
          const dy = (y - dragState.lastY) / zoom;

          setDragState(prev => (prev ? { ...prev, lastX: x, lastY: y } : null));

          const idsToMove = selectedAnnotationIds.includes(
            dragState.annotationId
          )
            ? selectedAnnotationIds
            : [dragState.annotationId];

          const updates: Array<{ id: string; updates: Partial<Annotation> }> =
            [];

          for (const id of idsToMove) {
            const annotation = annotations.find(a => a.id === id);
            if (annotation) {
              const moveUpdates = getAnnotationMoveUpdates(annotation, dx, dy);
              if (moveUpdates) {
                updates.push({ id, updates: moveUpdates });
              }
            }
          }

          if (updates.length > 0) {
            if (onAnnotationsUpdateMultiple && updates.length > 1) {
              onAnnotationsUpdateMultiple(updates);
            } else if (onAnnotationUpdate && updates.length === 1) {
              onAnnotationUpdate(updates[0].id, updates[0].updates);
            } else if (onAnnotationsUpdateMultiple) {
              onAnnotationsUpdateMultiple(updates);
            }
          }
        } else if (marqueeState) {
          const marqueeX = (x - 0) / zoom;
          const marqueeY = (y - 0) / zoom;

          setMarqueeState(prev =>
            prev ? { ...prev, currentX: marqueeX, currentY: marqueeY } : null
          );
        }
      };

      const handleWindowMouseUp = () => {
        if (dragState) {
          onDragEnd?.();
        }
        if (resizeState) {
          onDragEnd?.();
        }
        if (marqueeState && onSelectMultiple) {
          const marqueeBox: BoundingBox = {
            x: Math.min(marqueeState.startX, marqueeState.currentX) - offsetX,
            y: Math.min(marqueeState.startY, marqueeState.currentY) - offsetY,
            width: Math.abs(marqueeState.currentX - marqueeState.startX),
            height: Math.abs(marqueeState.currentY - marqueeState.startY),
          };

          if (marqueeBox.width > 5 || marqueeBox.height > 5) {
            const selectedIds = annotations
              .filter(ann => {
                const annBox = getBoundingBox(ann);
                return boxesIntersect(marqueeBox, annBox);
              })
              .map(ann => ann.id);

            onSelectMultiple(selectedIds);
          } else {
            onSelectMultiple([]);
          }
        }

        setDragState(null);
        setResizeState(null);
        setMarqueeState(null);
      };

      window.addEventListener('mousemove', handleWindowMouseMove);
      window.addEventListener('mouseup', handleWindowMouseUp);

      return () => {
        window.removeEventListener('mousemove', handleWindowMouseMove);
        window.removeEventListener('mouseup', handleWindowMouseUp);
      };
    }, [
      dragState,
      resizeState,
      marqueeState,
      zoom,
      annotations,
      selectedAnnotationIds,
      getAnnotationMoveUpdates,
      onAnnotationUpdate,
      onAnnotationsUpdateMultiple,
      onDragEnd,
      onSelectMultiple,
      offsetX,
      offsetY,
    ]);

    const handleSvgMouseDown = useCallback(
      (e: React.MouseEvent) => {
        if (activeTool !== 'select') return;
        if (e.target !== svgRef.current) return;

        const svg = svgRef.current;
        if (!svg) return;

        const rect = svg.getBoundingClientRect();
        const x = (e.clientX - rect.left) / zoom;
        const y = (e.clientY - rect.top) / zoom;

        setMarqueeState({
          startX: x,
          startY: y,
          currentX: x,
          currentY: y,
        });
      },
      [activeTool, zoom]
    );

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

        if (annotation.type === 'rectangle' || annotation.type === 'redact') {
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

    const handleCombinedMouseMove = useCallback(() => {}, []);

    const handleCombinedMouseUp = useCallback(() => {}, []);

    const renderResizeHandles = (ann: Annotation) => {
      if (
        selectedAnnotationIds.length !== 1 ||
        !selectedAnnotationIds.includes(ann.id)
      ) {
        return null;
      }

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
        case 'redact':
          return renderRedactHandles({ annotation: ann, ...commonProps });
        default:
          return null;
      }
    };

    const renderAnnotation = (ann: Annotation, isPreview = false) => {
      const isSelected = selectedAnnotationIds.includes(ann.id);

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
        case 'highlight':
          return renderHighlight({ annotation: ann, ...commonProps });
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
        case 'redact':
          return renderRedact({ annotation: ann, ...commonProps });
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
            case 'highlight':
              return '';
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
            case 'redact':
              return exportRedact({ annotation: ann, ...exportProps });
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

    useImperativeHandle(
      ref,
      () => ({
        getSvgForExport,
      }),
      [getSvgForExport]
    );

    const shouldEnablePointerEvents =
      activeTool === 'select' || dragState || resizeState || marqueeState;

    return (
      <svg
        ref={svgRef}
        width={width}
        height={height}
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          pointerEvents: shouldEnablePointerEvents ? 'auto' : 'none',
          overflow: 'visible',
        }}
        onMouseDown={handleSvgMouseDown}
        onMouseMove={handleCombinedMouseMove}
        onMouseUp={handleCombinedMouseUp}
      >
        <g>
          {annotations.map(ann => renderAnnotation(ann))}
          {currentAnnotation && renderAnnotation(currentAnnotation, true)}
          {annotations.map(ann => (
            <g key={`handles-${ann.id}`}>{renderResizeHandles(ann)}</g>
          ))}
        </g>
        {}
        {marqueeState && (
          <rect
            x={Math.min(marqueeState.startX, marqueeState.currentX)}
            y={Math.min(marqueeState.startY, marqueeState.currentY)}
            width={Math.abs(marqueeState.currentX - marqueeState.startX)}
            height={Math.abs(marqueeState.currentY - marqueeState.startY)}
            fill="rgba(59, 130, 246, 0.1)"
            stroke="#3b82f6"
            strokeWidth={1}
            strokeDasharray="4 2"
            pointerEvents="none"
          />
        )}
      </svg>
    );
  }
);

SvgAnnotationsOverlay.displayName = 'SvgAnnotationsOverlay';

export default SvgAnnotationsOverlay;
