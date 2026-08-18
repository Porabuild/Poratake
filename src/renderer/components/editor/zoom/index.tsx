import { Minus, Plus } from 'lucide-react';

import { Button } from '@/renderer/components/ui/button';

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

  // `rounded-3xl` is the button radius here: HeroUI maps the radius scale onto --radius.
  return (
    <div
      className="fixed right-4 bottom-4 flex items-center gap-0.5 rounded-3xl bg-surface/90 p-1 shadow-lg backdrop-blur-md"
      style={{ zIndex: 9999999 }}
    >
      <Button
        variant="ghost"
        size="icon-xs"
        onClick={onZoomOut}
        disabled={zoom <= MIN_ZOOM}
        title="Zoom Out"
      >
        <Minus className="size-3.5" />
      </Button>
      <Button
        variant="ghost"
        size="xs"
        onClick={onZoomReset}
        title="Reset Zoom"
        className="min-w-14 text-muted-foreground tabular-nums"
      >
        {zoomPercentage}%
      </Button>
      <Button
        variant="ghost"
        size="icon-xs"
        onClick={onZoomIn}
        disabled={zoom >= MAX_ZOOM}
        title="Zoom In"
      >
        <Plus className="size-3.5" />
      </Button>
    </div>
  );
}
