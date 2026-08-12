import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

interface CropRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface SvgCropOverlayProps {
  cropRect: CropRect;
  canvasWidth: number;
  canvasHeight: number;
  onCropRectChange: (rect: CropRect) => void;
  offsetX?: number;
  offsetY?: number;
  imageWidth: number;
  imageHeight: number;
}

type ResizeHandle = 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';

interface DragState {
  type: 'move' | 'resize';
  handle?: ResizeHandle;
  startX: number;
  startY: number;
  startRect: CropRect;
}

const HANDLE_SIZE = 12;
const MIN_SIZE = 20;

const SvgCropOverlay = ({
  cropRect,
  canvasWidth,
  canvasHeight,
  onCropRectChange,
  offsetX = 0,
  offsetY = 0,
  imageWidth,
  imageHeight,
}: SvgCropOverlayProps) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const [dragState, setDragState] = useState<DragState | null>(null);

  const normalizedRect = useMemo(
    () => ({
      x: cropRect.width < 0 ? cropRect.x + cropRect.width : cropRect.x,
      y: cropRect.height < 0 ? cropRect.y + cropRect.height : cropRect.y,
      width: Math.abs(cropRect.width),
      height: Math.abs(cropRect.height),
    }),
    [cropRect.x, cropRect.y, cropRect.width, cropRect.height]
  );

  const getMousePosition = useCallback(
    (e: React.MouseEvent): { x: number; y: number } => {
      const svg = svgRef.current;
      if (!svg) return { x: 0, y: 0 };

      const pt = svg.createSVGPoint();
      pt.x = e.clientX;
      pt.y = e.clientY;

      const ctm = svg.getScreenCTM();
      if (!ctm) return { x: 0, y: 0 };

      const transformedPt = pt.matrixTransform(ctm.inverse());

      return {
        x: transformedPt.x - offsetX,
        y: transformedPt.y - offsetY,
      };
    },
    [offsetX, offsetY]
  );

  const handleMoveStart = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      e.preventDefault();

      const pos = getMousePosition(e);
      setDragState({
        type: 'move',
        startX: pos.x,
        startY: pos.y,
        startRect: { ...normalizedRect },
      });
    },
    [normalizedRect, getMousePosition]
  );

  const handleResizeStart = useCallback(
    (e: React.MouseEvent, handle: ResizeHandle) => {
      e.stopPropagation();
      e.preventDefault();

      const pos = getMousePosition(e);
      setDragState({
        type: 'resize',
        handle,
        startX: pos.x,
        startY: pos.y,
        startRect: { ...normalizedRect },
      });
    },
    [normalizedRect, getMousePosition]
  );

  const handleMouseMove = useCallback(() => {}, []);

  const handleMouseUp = useCallback(() => {}, []);

  useEffect(() => {
    if (!dragState) return;

    const svg = svgRef.current;
    if (!svg) return;

    const handleWindowMouseMove = (e: MouseEvent) => {
      const pt = svg.createSVGPoint();
      pt.x = e.clientX;
      pt.y = e.clientY;

      const ctm = svg.getScreenCTM();
      if (!ctm) return;

      const transformedPt = pt.matrixTransform(ctm.inverse());

      const pos = {
        x: transformedPt.x - offsetX,
        y: transformedPt.y - offsetY,
      };

      const dx = pos.x - dragState.startX;
      const dy = pos.y - dragState.startY;

      if (dragState.type === 'move') {
        const { startRect } = dragState;
        let newX = startRect.x + dx;
        let newY = startRect.y + dy;

        newX = Math.max(0, Math.min(newX, imageWidth - startRect.width));
        newY = Math.max(0, Math.min(newY, imageHeight - startRect.height));

        onCropRectChange({
          x: newX,
          y: newY,
          width: startRect.width,
          height: startRect.height,
        });
      } else if (dragState.type === 'resize' && dragState.handle) {
        const { startRect, handle } = dragState;
        const newRect = { ...startRect };

        switch (handle) {
          case 'top-left': {
            let newWidth = startRect.width - dx;
            let newHeight = startRect.height - dy;

            if (newWidth < MIN_SIZE) {
              newWidth = MIN_SIZE;
              newRect.x = startRect.x + startRect.width - MIN_SIZE;
            } else if (startRect.x + dx < 0) {
              newRect.x = 0;
              newWidth = startRect.x + startRect.width;
            } else {
              newRect.x = startRect.x + dx;
            }
            newRect.width = newWidth;

            if (newHeight < MIN_SIZE) {
              newHeight = MIN_SIZE;
              newRect.y = startRect.y + startRect.height - MIN_SIZE;
            } else if (startRect.y + dy < 0) {
              newRect.y = 0;
              newHeight = startRect.y + startRect.height;
            } else {
              newRect.y = startRect.y + dy;
            }
            newRect.height = newHeight;
            break;
          }
          case 'top-right': {
            let newWidth = startRect.width + dx;
            let newHeight = startRect.height - dy;

            if (newWidth < MIN_SIZE) {
              newWidth = MIN_SIZE;
            } else if (startRect.x + newWidth > imageWidth) {
              newWidth = imageWidth - startRect.x;
            }
            newRect.width = newWidth;

            if (newHeight < MIN_SIZE) {
              newHeight = MIN_SIZE;
              newRect.y = startRect.y + startRect.height - MIN_SIZE;
            } else if (startRect.y + dy < 0) {
              newRect.y = 0;
              newHeight = startRect.y + startRect.height;
            } else {
              newRect.y = startRect.y + dy;
            }
            newRect.height = newHeight;
            break;
          }
          case 'bottom-left': {
            let newWidth = startRect.width - dx;
            let newHeight = startRect.height + dy;

            if (newWidth < MIN_SIZE) {
              newWidth = MIN_SIZE;
              newRect.x = startRect.x + startRect.width - MIN_SIZE;
            } else if (startRect.x + dx < 0) {
              newRect.x = 0;
              newWidth = startRect.x + startRect.width;
            } else {
              newRect.x = startRect.x + dx;
            }
            newRect.width = newWidth;

            if (newHeight < MIN_SIZE) {
              newHeight = MIN_SIZE;
            } else if (startRect.y + newHeight > imageHeight) {
              newHeight = imageHeight - startRect.y;
            }
            newRect.height = newHeight;
            break;
          }
          case 'bottom-right': {
            let newWidth = startRect.width + dx;
            let newHeight = startRect.height + dy;

            if (newWidth < MIN_SIZE) {
              newWidth = MIN_SIZE;
            } else if (startRect.x + newWidth > imageWidth) {
              newWidth = imageWidth - startRect.x;
            }
            newRect.width = newWidth;

            if (newHeight < MIN_SIZE) {
              newHeight = MIN_SIZE;
            } else if (startRect.y + newHeight > imageHeight) {
              newHeight = imageHeight - startRect.y;
            }
            newRect.height = newHeight;
            break;
          }
        }

        onCropRectChange(newRect);
      }
    };

    const handleWindowMouseUp = () => {
      setDragState(null);
    };

    window.addEventListener('mousemove', handleWindowMouseMove);
    window.addEventListener('mouseup', handleWindowMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleWindowMouseMove);
      window.removeEventListener('mouseup', handleWindowMouseUp);
    };
  }, [dragState, offsetX, offsetY, imageWidth, imageHeight, onCropRectChange]);

  const handleStyle = {
    fill: 'white',
    stroke: '#007AFF',
    strokeWidth: 2,
    cursor: 'default',
  };

  const handles: { pos: ResizeHandle; x: number; y: number; cursor: string }[] =
    [
      {
        pos: 'top-left',
        x: normalizedRect.x + offsetX,
        y: normalizedRect.y + offsetY,
        cursor: 'nwse-resize',
      },
      {
        pos: 'top-right',
        x: normalizedRect.x + normalizedRect.width + offsetX,
        y: normalizedRect.y + offsetY,
        cursor: 'nesw-resize',
      },
      {
        pos: 'bottom-left',
        x: normalizedRect.x + offsetX,
        y: normalizedRect.y + normalizedRect.height + offsetY,
        cursor: 'nesw-resize',
      },
      {
        pos: 'bottom-right',
        x: normalizedRect.x + normalizedRect.width + offsetX,
        y: normalizedRect.y + normalizedRect.height + offsetY,
        cursor: 'nwse-resize',
      },
    ];

  const textX = normalizedRect.x + normalizedRect.width / 2 + offsetX;
  const textY = normalizedRect.y + normalizedRect.height + 24 + offsetY;

  return (
    <svg
      ref={svgRef}
      width={canvasWidth}
      height={canvasHeight}
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        pointerEvents: dragState ? 'auto' : 'none',
        overflow: 'visible',
      }}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
    >
      <defs>
        <mask id="crop-mask">
          <rect
            x={0}
            y={0}
            width={canvasWidth}
            height={canvasHeight}
            fill="white"
          />
          <rect
            x={normalizedRect.x + offsetX}
            y={normalizedRect.y + offsetY}
            width={normalizedRect.width}
            height={normalizedRect.height}
            fill="black"
          />
        </mask>
      </defs>

      <rect
        x={0}
        y={0}
        width={canvasWidth}
        height={canvasHeight}
        fill="rgba(0, 0, 0, 0.5)"
        mask="url(#crop-mask)"
        style={{ pointerEvents: 'none' }}
      />

      <rect
        x={normalizedRect.x + offsetX}
        y={normalizedRect.y + offsetY}
        width={normalizedRect.width}
        height={normalizedRect.height}
        fill="transparent"
        stroke="#007AFF"
        strokeWidth={2}
        rx={1}
        style={{ cursor: 'move', pointerEvents: 'auto' }}
        onMouseDown={handleMoveStart}
      />

      {handles.map(({ pos, x, y, cursor }) => (
        <rect
          key={pos}
          x={x - HANDLE_SIZE / 2}
          y={y - HANDLE_SIZE / 2}
          width={HANDLE_SIZE}
          height={HANDLE_SIZE}
          rx={2}
          {...handleStyle}
          style={{ cursor, pointerEvents: 'auto' }}
          onMouseDown={e => handleResizeStart(e, pos)}
        />
      ))}

      <text
        x={textX}
        y={textY}
        textAnchor="middle"
        fill="#007AFF"
        fontSize={14}
        fontFamily="system-ui, -apple-system, sans-serif"
        style={{ pointerEvents: 'none', userSelect: 'none' }}
      >
        Press Enter to crop
      </text>
    </svg>
  );
};

export default SvgCropOverlay;
