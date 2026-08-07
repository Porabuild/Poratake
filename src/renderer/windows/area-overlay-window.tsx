import { useCallback, useEffect, useRef, useState } from 'react';
import type { AreaOverlayParams } from '@/types/area-overlay';

interface SelectionRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface DragState {
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
}

const MIN_SELECTION_SIZE = 4;

function toRect(drag: DragState): SelectionRect {
  return {
    x: Math.min(drag.startX, drag.currentX),
    y: Math.min(drag.startY, drag.currentY),
    width: Math.abs(drag.currentX - drag.startX),
    height: Math.abs(drag.currentY - drag.startY),
  };
}

export default function AreaOverlayWindow({
  params,
}: {
  params: AreaOverlayParams;
}) {
  const [drag, setDrag] = useState<DragState | null>(null);
  const frozenFrame = useRef<HTMLImageElement>(null);

  const rect = drag ? toRect(drag) : null;
  const hasSelection =
    rect !== null &&
    rect.width >= MIN_SELECTION_SIZE &&
    rect.height >= MIN_SELECTION_SIZE;

  const confirmSelection = useCallback(
    (selection: SelectionRect) => {
      window.ipcRenderer.send('area-overlay:confirm', {
        displayId: params.displayId,
        ...selection,
      });
    },
    [params.displayId]
  );

  useEffect(() => {
    const announceReady = () =>
      window.ipcRenderer.send('area-overlay:ready', params.displayId);
    const image = frozenFrame.current;

    if (!image) {
      announceReady();
      return;
    }

    image.decode().then(announceReady, announceReady);
  }, [params.displayId, params.imageUrl]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        window.ipcRenderer.send('area-overlay:cancel');
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    setDrag({
      startX: e.clientX,
      startY: e.clientY,
      currentX: e.clientX,
      currentY: e.clientY,
    });
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (!drag) return;
    setDrag({ ...drag, currentX: e.clientX, currentY: e.clientY });
  };

  const handleMouseUp = () => {
    if (!drag) return;
    const selection = toRect(drag);
    setDrag(null);
    if (
      selection.width >= MIN_SELECTION_SIZE &&
      selection.height >= MIN_SELECTION_SIZE
    ) {
      confirmSelection(selection);
    }
  };

  return (
    <div
      className="fixed inset-0 cursor-crosshair overflow-hidden select-none"
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
    >
      <img
        ref={frozenFrame}
        src={params.imageUrl}
        className="pointer-events-none absolute inset-0 h-full w-full"
        alt=""
        draggable={false}
      />
      {rect && hasSelection ? (
        <div
          className="pointer-events-none absolute border border-white/90"
          style={{
            left: rect.x,
            top: rect.y,
            width: rect.width,
            height: rect.height,
            boxShadow: '0 0 0 100000px rgba(0, 0, 0, 0.45)',
          }}
        >
          <div className="absolute -bottom-7 left-0 rounded bg-black/70 px-2 py-0.5 font-mono text-xs whitespace-nowrap text-white">
            {rect.width} × {rect.height}
          </div>
        </div>
      ) : (
        <div className="pointer-events-none absolute inset-0 bg-black/45">
          <div className="absolute top-6 left-1/2 -translate-x-1/2 rounded bg-black/70 px-3 py-1 text-sm text-white">
            Drag to select an area — Esc to cancel
          </div>
        </div>
      )}
    </div>
  );
}
