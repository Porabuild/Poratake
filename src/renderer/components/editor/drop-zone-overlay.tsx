import type { DropEdge } from '@/renderer/hooks/useImageDrop';

interface DropZoneOverlayProps {
  isDragging: boolean;
  dropEdge: DropEdge;
}

const EDGE_STYLES: Record<
  NonNullable<DropEdge>,
  { zone: string; indicator: string }
> = {
  top: {
    zone: 'inset-x-0 top-0 h-1/4',
    indicator: 'top-0 inset-x-0 h-1 rounded-b',
  },
  bottom: {
    zone: 'inset-x-0 bottom-0 h-1/4',
    indicator: 'bottom-0 inset-x-0 h-1 rounded-t',
  },
  left: {
    zone: 'inset-y-0 left-0 w-1/4',
    indicator: 'left-0 inset-y-0 w-1 rounded-r',
  },
  right: {
    zone: 'inset-y-0 right-0 w-1/4',
    indicator: 'right-0 inset-y-0 w-1 rounded-l',
  },
};

export default function DropZoneOverlay({
  isDragging,
  dropEdge,
}: DropZoneOverlayProps) {
  if (!isDragging) return null;

  return (
    <div className="pointer-events-none absolute inset-0 z-50">
      <div className="absolute inset-0 rounded-lg border-2 border-dashed border-blue-400/50 bg-blue-500/5" />
      {dropEdge && (
        <>
          <div
            className={`absolute ${EDGE_STYLES[dropEdge].zone} bg-blue-500/15 transition-all duration-150`}
          />
          <div
            className={`absolute ${EDGE_STYLES[dropEdge].indicator} bg-blue-500 transition-all duration-150`}
          />
        </>
      )}
    </div>
  );
}
