import { Minus, Plus } from 'lucide-react';

export const MIN_ZOOM = 0.25;
export const MAX_ZOOM = 4;
export const MAX_FIT_ZOOM = 2;
export const ZOOM_STEP = 0.1;

interface ZoomControlProps {
  zoom: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onZoomReset: () => void;
}

export default function ZoomControl({
  zoom,
  onZoomIn,
  onZoomOut,
  onZoomReset,
}: ZoomControlProps) {
  const zoomPercentage = Math.round(zoom * 100);

  return (
    <div
      className="bg-background border-border fixed right-4 bottom-4 flex items-center gap-1 rounded-lg border p-1 shadow-lg"
      style={{ zIndex: 9999999 }}
    >
      <button
        onClick={onZoomOut}
        className="hover:bg-accent flex size-7 items-center justify-center rounded transition-colors disabled:opacity-50"
        disabled={zoom <= MIN_ZOOM}
        title="Zoom Out"
      >
        <Minus className="size-4" />
      </button>
      <button
        onClick={onZoomReset}
        className="hover:bg-accent text-muted-foreground min-w-14 rounded px-2 py-1 text-xs font-medium transition-colors"
        title="Reset Zoom"
      >
        {zoomPercentage}%
      </button>
      <button
        onClick={onZoomIn}
        className="hover:bg-accent flex size-7 items-center justify-center rounded transition-colors disabled:opacity-50"
        disabled={zoom >= MAX_ZOOM}
        title="Zoom In"
      >
        <Plus className="size-4" />
      </button>
    </div>
  );
}
