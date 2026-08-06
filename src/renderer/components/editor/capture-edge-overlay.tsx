import { Camera } from 'lucide-react';
import type { DropEdge } from '@/renderer/hooks/useImageDrop';

interface CaptureEdgeOverlayProps {
  isActive: boolean;
  onEdgeClick: (edge: NonNullable<DropEdge>) => void;
}

const EDGES: NonNullable<DropEdge>[] = ['top', 'bottom', 'left', 'right'];

const EDGE_CONFIG: Record<
  NonNullable<DropEdge>,
  { zone: string; indicator: string; pill: string }
> = {
  top: {
    zone: 'inset-x-0 top-0 h-8',
    indicator: 'top-0 inset-x-0 h-px',
    pill: 'top-2 left-1/2 -translate-x-1/2',
  },
  bottom: {
    zone: 'inset-x-0 bottom-0 h-8',
    indicator: 'bottom-0 inset-x-0 h-px',
    pill: 'bottom-2 left-1/2 -translate-x-1/2',
  },
  left: {
    zone: 'inset-y-0 left-0 w-8',
    indicator: 'left-0 inset-y-0 w-px',
    pill: 'left-2 top-1/2 -translate-y-1/2',
  },
  right: {
    zone: 'inset-y-0 right-0 w-8',
    indicator: 'right-0 inset-y-0 w-px',
    pill: 'right-2 top-1/2 -translate-y-1/2',
  },
};

export default function CaptureEdgeOverlay({
  isActive,
  onEdgeClick,
}: CaptureEdgeOverlayProps) {
  if (!isActive) return null;

  return (
    <div className="absolute inset-0 z-50">
      {EDGES.map(edge => (
        <button
          key={edge}
          type="button"
          className={`group absolute ${EDGE_CONFIG[edge].zone} cursor-pointer`}
          onMouseDown={e => {
            if (e.button !== 0 && e.button !== 2) return;
            e.preventDefault();
            onEdgeClick(edge);
          }}
          onContextMenu={e => e.preventDefault()}
        >
          <div
            className={`absolute ${EDGE_CONFIG[edge].indicator} bg-primary/0 group-hover:bg-primary transition-colors duration-150`}
          />
          <div
            className={`absolute ${EDGE_CONFIG[edge].pill} pointer-events-none`}
          >
            <div className="bg-primary text-primary-foreground flex size-6 items-center justify-center rounded-full opacity-40 shadow-sm transition-opacity duration-150 group-hover:opacity-100">
              <Camera className="size-3" />
            </div>
          </div>
        </button>
      ))}
    </div>
  );
}
