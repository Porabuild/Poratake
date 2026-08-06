import { useState, useCallback } from 'react';
import type {
  Annotation,
  ArrowStyle,
  HighlightColor,
  HighlightOpacity,
  RedactIntensity,
  RedactStyle,
  ShapeFillMode,
  ToolType,
} from '@/types/editor';

interface UseDrawingToolsProps {
  activeTool: ToolType;
  selectedColor: string;
  strokeWidth: number;
  arrowStyle?: ArrowStyle;
  highlightColor?: HighlightColor;
  highlightOpacity?: HighlightOpacity;
  redactStyle?: RedactStyle;
  redactIntensity?: RedactIntensity;
  shapeFillMode?: ShapeFillMode;
  onAnnotationAdd: (annotation: Annotation) => void;
}

const MIN_DRAW_DISTANCE = 5;

export const useDrawingTools = ({
  activeTool,
  selectedColor,
  strokeWidth,
  arrowStyle = 'standard',
  highlightColor = '#FFFF00',
  highlightOpacity = 0.4,
  redactStyle = 'pixelate',
  redactIntensity = 5,
  shapeFillMode = 'outline',
  onAnnotationAdd,
}: UseDrawingToolsProps) => {
  const [isDrawing, setIsDrawing] = useState(false);
  const [currentAnnotation, setCurrentAnnotation] = useState<Annotation | null>(
    null
  );
  const [startPosition, setStartPosition] = useState<{
    x: number;
    y: number;
  } | null>(null);

  const createAnnotation = useCallback(
    (pos: { x: number; y: number }): Annotation | null => {
      const timestamp = Date.now();

      switch (activeTool) {
        case 'pen':
          return {
            id: `pen-${timestamp}`,
            type: 'pen',
            points: [pos.x, pos.y],
            stroke: selectedColor,
            strokeWidth,
          };
        case 'highlight':
          return {
            id: `highlight-${timestamp}`,
            type: 'highlight',
            points: [pos.x, pos.y],
            fill: highlightColor,
            opacity: highlightOpacity,
            strokeWidth: 20,
          };
        case 'rectangle':
          return {
            id: `rect-${timestamp}`,
            type: 'rectangle',
            x: pos.x,
            y: pos.y,
            width: 0,
            height: 0,
            stroke: selectedColor,
            strokeWidth,
            fill: shapeFillMode === 'filled' ? selectedColor : undefined,
          };
        case 'circle':
          return {
            id: `circle-${timestamp}`,
            type: 'circle',
            x: pos.x,
            y: pos.y,
            radius: 0,
            stroke: selectedColor,
            strokeWidth,
            fill: shapeFillMode === 'filled' ? selectedColor : undefined,
            startX: pos.x,
            startY: pos.y,
          } as Annotation;
        case 'line':
          return {
            id: `line-${timestamp}`,
            type: 'line',
            points: [pos.x, pos.y, pos.x, pos.y],
            stroke: selectedColor,
            strokeWidth,
          };
        case 'arrow':
          return {
            id: `arrow-${timestamp}`,
            type: 'arrow',
            points: [pos.x, pos.y, pos.x, pos.y],
            stroke: selectedColor,
            strokeWidth,
            arrowStyle,
          };
        case 'redact':
          return {
            id: `redact-${timestamp}`,
            type: 'redact',
            x: pos.x,
            y: pos.y,
            width: 0,
            height: 0,
            style: redactStyle,
            intensity: redactIntensity,
          };
        default:
          return null;
      }
    },
    [
      activeTool,
      selectedColor,
      strokeWidth,
      arrowStyle,
      highlightColor,
      highlightOpacity,
      redactStyle,
      redactIntensity,
      shapeFillMode,
    ]
  );

  const updateAnnotation = useCallback(
    (pos: { x: number; y: number }, shiftKey = false) => {
      if (!currentAnnotation && startPosition) {
        const distance = Math.sqrt(
          Math.pow(pos.x - startPosition.x, 2) +
            Math.pow(pos.y - startPosition.y, 2)
        );

        if (distance >= MIN_DRAW_DISTANCE) {
          const annotation = createAnnotation(startPosition);
          if (annotation) {
            setCurrentAnnotation(annotation);
          }
        } else {
          return;
        }
      }

      if (!currentAnnotation) return;

      switch (currentAnnotation.type) {
        case 'pen':
        case 'highlight': {
          let finalX = pos.x;
          let finalY = pos.y;

          if (shiftKey && currentAnnotation.points.length >= 2) {
            const startX = currentAnnotation.points[0];
            const startY = currentAnnotation.points[1];

            const dx = Math.abs(pos.x - startX);
            const dy = Math.abs(pos.y - startY);

            if (dx > dy) {
              finalY = startY;
            } else {
              finalX = startX;
            }

            const points: number[] = [startX, startY];
            const distance = Math.sqrt(
              Math.pow(finalX - startX, 2) + Math.pow(finalY - startY, 2)
            );
            const numPoints = Math.max(2, Math.floor(distance / 5));

            for (let i = 1; i <= numPoints; i++) {
              const t = i / numPoints;
              points.push(startX + (finalX - startX) * t);
              points.push(startY + (finalY - startY) * t);
            }

            setCurrentAnnotation({
              ...currentAnnotation,
              points,
            });
          } else {
            setCurrentAnnotation({
              ...currentAnnotation,
              points: [...currentAnnotation.points, finalX, finalY],
            });
          }
          break;
        }
        case 'rectangle':
        case 'redact':
          setCurrentAnnotation({
            ...currentAnnotation,
            width: pos.x - currentAnnotation.x,
            height: pos.y - currentAnnotation.y,
          });
          break;
        case 'circle': {
          const circleAnnotation = currentAnnotation as Annotation & {
            startX?: number;
            startY?: number;
          };
          const startX = circleAnnotation.startX ?? currentAnnotation.x;
          const startY = circleAnnotation.startY ?? currentAnnotation.y;

          const centerX = (startX + pos.x) / 2;
          const centerY = (startY + pos.y) / 2;

          const radius =
            Math.sqrt(
              Math.pow(pos.x - startX, 2) + Math.pow(pos.y - startY, 2)
            ) / 2;

          setCurrentAnnotation({
            ...currentAnnotation,
            x: centerX,
            y: centerY,
            radius,
          });
          break;
        }
        case 'line':
        case 'arrow':
          setCurrentAnnotation({
            ...currentAnnotation,
            points: [
              currentAnnotation.points[0],
              currentAnnotation.points[1],
              pos.x,
              pos.y,
            ],
          });
          break;
      }
    },
    [currentAnnotation, startPosition, createAnnotation]
  );

  const startDrawing = useCallback(
    (pos: { x: number; y: number }) => {
      setIsDrawing(true);

      if (activeTool === 'arrow' || activeTool === 'line') {
        setStartPosition(pos);
        setCurrentAnnotation(null);
      } else {
        const annotation = createAnnotation(pos);
        if (annotation) {
          setCurrentAnnotation(annotation);
        }
        setStartPosition(null);
      }
    },
    [createAnnotation, activeTool]
  );

  const finishDrawing = useCallback(() => {
    if (currentAnnotation) {
      const finalAnnotation = { ...currentAnnotation };
      const cleaned = finalAnnotation as Annotation & {
        startX?: number;
        startY?: number;
      };
      if ('startX' in cleaned) delete cleaned.startX;
      if ('startY' in cleaned) delete cleaned.startY;

      onAnnotationAdd(finalAnnotation);
      setCurrentAnnotation(null);
    }
    setIsDrawing(false);
    setStartPosition(null);
  }, [currentAnnotation, onAnnotationAdd]);

  return {
    isDrawing,
    currentAnnotation,
    startDrawing,
    updateAnnotation,
    finishDrawing,
    setIsDrawing,
  };
};
