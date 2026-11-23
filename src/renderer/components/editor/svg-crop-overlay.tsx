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
  zoom?: number;
}

type ResizeHandle = 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';

interface DragState {
  type: 'move' | 'resize';
  handle?: ResizeHandle;
  startX: number;
  startY: number;
  startRect: CropRect;
}

const HANDLE_SIZE = 8;
const HIT_HANDLE_SIZE = 14;
const HINT_GAP = 10;
const HINT_HEIGHT = 28;
const HINT_EST_WIDTH = 136;
const HINT_EDGE_PAD = 8;
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
  zoom = 1,
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

  const invZoom = zoom > 0 ? 1 / zoom : 1;
  const strokeWidth = invZoom;
  const handleSize = HANDLE_SIZE * invZoom;
  const hitHandleSize = HIT_HANDLE_SIZE * invZoom;
  const handleRadius = 2 * invZoom;

  const handleStyle = {
    fill: '#ffffff',
    stroke: 'rgba(0, 0, 0, 0.45)',
    strokeWidth,
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

  const boxLeft = normalizedRect.x + offsetX;
  const boxTop = normalizedRect.y + offsetY;
  const hintGap = HINT_GAP * invZoom;
  const hintHeight = HINT_HEIGHT * invZoom;
  const hintHalfW = HINT_EST_WIDTH * 0.5 * invZoom;
  const hintEdgePad = HINT_EDGE_PAD * invZoom;
  const rawHintX = boxLeft + normalizedRect.width / 2;
  const hintX =
    canvasWidth > (HINT_EST_WIDTH + HINT_EDGE_PAD * 2) * invZoom
      ? Math.max(
          hintHalfW + hintEdgePad,
          Math.min(rawHintX, canvasWidth - hintHalfW - hintEdgePad)
        )
      : canvasWidth / 2;
  const belowY = boxTop + normalizedRect.height + hintGap;
  let hintY = belowY;
  if (belowY + hintHeight > canvasHeight) {
    if (boxTop >= hintGap + hintHeight) {
      hintY = boxTop - hintGap - hintHeight;
    } else {
      hintY = Math.max(
        boxTop + hintGap,
        boxTop + normalizedRect.height - hintGap - hintHeight
      );
    }
  }
  const showHint = normalizedRect.width >= 2 && normalizedRect.height >= 2;

  return (
    <div
      style={{
        position: 'absolute',
        inset: 0,
        pointerEvents: 'none',
        zIndex: 30,
      }}
    >
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
              x={boxLeft}
              y={boxTop}
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
          x={boxLeft}
          y={boxTop}
          width={normalizedRect.width}
          height={normalizedRect.height}
          fill="transparent"
          stroke="rgba(255, 255, 255, 0.85)"
          strokeWidth={strokeWidth}
          rx={invZoom}
          style={{ cursor: 'move', pointerEvents: 'auto' }}
          onMouseDown={handleMoveStart}
        />

        {handles.map(({ pos, x, y, cursor }) => (
          <g key={pos}>
            <rect
              x={x - handleSize / 2}
              y={y - handleSize / 2}
              width={handleSize}
              height={handleSize}
              rx={handleRadius}
              {...handleStyle}
              style={{ pointerEvents: 'none' }}
            />
            <rect
              x={x - hitHandleSize / 2}
              y={y - hitHandleSize / 2}
              width={hitHandleSize}
              height={hitHandleSize}
              fill="transparent"
              style={{ cursor, pointerEvents: 'auto' }}
              onMouseDown={e => handleResizeStart(e, pos)}
            />
          </g>
        ))}
      </svg>

      {showHint && (
        <div
          className="pointer-events-none absolute rounded-md border border-white/15 bg-black/75 px-2 py-1 text-xs font-medium whitespace-nowrap text-white/90 shadow-sm select-none"
          style={{
            left: hintX,
            top: hintY,
            transform: `translate(-50%, 0) scale(${invZoom})`,
            transformOrigin: 'top center',
          }}
        >
          Press Enter to crop
        </div>
      )}
    </div>
  );
};

export default SvgCropOverlay;
