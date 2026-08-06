import { useState, useCallback, useEffect } from 'react';
import type { Annotation } from '@/types/editor';

interface UseCropToolProps {
  annotations: Annotation[];
  onCrop?: (
    cropData: { x: number; y: number; width: number; height: number },
    adjustedAnnotations: Annotation[]
  ) => void;
  contentWidth?: number;
  contentHeight?: number;
}

export const useCropTool = ({
  annotations,
  onCrop,
  contentWidth = 0,
  contentHeight = 0,
}: UseCropToolProps) => {
  const [cropRect, setCropRect] = useState<{
    x: number;
    y: number;
    width: number;
    height: number;
  } | null>(null);

  const clampToContent = useCallback(
    (x: number, y: number) => ({
      x: Math.max(0, Math.min(x, contentWidth)),
      y: Math.max(0, Math.min(y, contentHeight)),
    }),
    [contentWidth, contentHeight]
  );

  const startCrop = useCallback(
    (pos: { x: number; y: number }) => {
      const clamped = clampToContent(pos.x, pos.y);
      setCropRect({
        x: clamped.x,
        y: clamped.y,
        width: 0,
        height: 0,
      });
    },
    [clampToContent]
  );

  const updateCrop = useCallback(
    (pos: { x: number; y: number }) => {
      if (!cropRect) return;

      const clamped = clampToContent(pos.x, pos.y);

      setCropRect({
        ...cropRect,
        width: clamped.x - cropRect.x,
        height: clamped.y - cropRect.y,
      });
    },
    [cropRect, clampToContent]
  );

  const adjustAnnotationsForCrop = useCallback(
    (
      actualX: number,
      actualY: number,
      actualWidth: number,
      actualHeight: number
    ): Annotation[] => {
      return annotations
        .map(ann => {
          switch (ann.type) {
            case 'pen': {
              const adjustedPoints = ann.points.map((val, idx) =>
                idx % 2 === 0 ? val - actualX : val - actualY
              );
              const isInBounds = adjustedPoints.some((val, idx) => {
                if (idx % 2 === 0) return val >= 0 && val <= actualWidth;
                return val >= 0 && val <= actualHeight;
              });
              return isInBounds ? { ...ann, points: adjustedPoints } : null;
            }

            case 'rectangle': {
              const newRectX = ann.x - actualX;
              const newRectY = ann.y - actualY;
              if (
                newRectX + ann.width < 0 ||
                newRectX > actualWidth ||
                newRectY + ann.height < 0 ||
                newRectY > actualHeight
              ) {
                return null;
              }
              return { ...ann, x: newRectX, y: newRectY };
            }

            case 'circle': {
              const newCircleX = ann.x - actualX;
              const newCircleY = ann.y - actualY;
              if (
                newCircleX + ann.radius < 0 ||
                newCircleX - ann.radius > actualWidth ||
                newCircleY + ann.radius < 0 ||
                newCircleY - ann.radius > actualHeight
              ) {
                return null;
              }
              return { ...ann, x: newCircleX, y: newCircleY };
            }

            case 'line':
            case 'arrow': {
              const adjustedLinePoints: [number, number, number, number] = [
                ann.points[0] - actualX,
                ann.points[1] - actualY,
                ann.points[2] - actualX,
                ann.points[3] - actualY,
              ];
              const lineInBounds =
                (adjustedLinePoints[0] >= 0 &&
                  adjustedLinePoints[0] <= actualWidth) ||
                (adjustedLinePoints[2] >= 0 &&
                  adjustedLinePoints[2] <= actualWidth) ||
                (adjustedLinePoints[1] >= 0 &&
                  adjustedLinePoints[1] <= actualHeight) ||
                (adjustedLinePoints[3] >= 0 &&
                  adjustedLinePoints[3] <= actualHeight);
              return lineInBounds
                ? { ...ann, points: adjustedLinePoints }
                : null;
            }

            case 'text': {
              const newTextX = ann.x - actualX;
              const newTextY = ann.y - actualY;
              if (
                newTextX < 0 ||
                newTextX > actualWidth ||
                newTextY < 0 ||
                newTextY > actualHeight
              ) {
                return null;
              }
              return { ...ann, x: newTextX, y: newTextY };
            }

            case 'number': {
              const newNumberX = ann.x - actualX;
              const newNumberY = ann.y - actualY;
              const radiusEstimate = 24;
              if (
                newNumberX + radiusEstimate < 0 ||
                newNumberX - radiusEstimate > actualWidth ||
                newNumberY + radiusEstimate < 0 ||
                newNumberY - radiusEstimate > actualHeight
              ) {
                return null;
              }
              return { ...ann, x: newNumberX, y: newNumberY };
            }

            default:
              return ann;
          }
        })
        .filter((ann): ann is Annotation => ann !== null);
    },
    [annotations]
  );

  const applyCrop = useCallback(() => {
    if (!cropRect || !onCrop) return;

    const actualX =
      cropRect.width < 0 ? cropRect.x + cropRect.width : cropRect.x;
    const actualY =
      cropRect.height < 0 ? cropRect.y + cropRect.height : cropRect.y;
    const actualWidth = Math.abs(cropRect.width);
    const actualHeight = Math.abs(cropRect.height);

    const adjustedAnnotations = adjustAnnotationsForCrop(
      actualX,
      actualY,
      actualWidth,
      actualHeight
    );

    onCrop(
      {
        x: actualX,
        y: actualY,
        width: actualWidth,
        height: actualHeight,
      },
      adjustedAnnotations
    );
    setCropRect(null);
  }, [cropRect, onCrop, adjustAnnotationsForCrop]);

  const cancelCrop = useCallback(() => {
    setCropRect(null);
  }, []);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (cropRect && e.key === 'Enter') {
        applyCrop();
      } else if (cropRect && e.key === 'Escape') {
        cancelCrop();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [cropRect, applyCrop, cancelCrop]);

  const setCropRectDirect = useCallback(
    (rect: { x: number; y: number; width: number; height: number }) => {
      setCropRect(rect);
    },
    []
  );

  return {
    cropRect,
    startCrop,
    updateCrop,
    setCropRect: setCropRectDirect,
  };
};
