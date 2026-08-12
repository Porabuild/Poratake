import { useCallback, useState } from 'react';
import { Camera, Trash2 } from 'lucide-react';
import type { CameraSegment } from '@/types/camera';
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from '@/renderer/components/ui/context-menu';
import Track, { type TrackSegment } from './track';
import TrackRow from './track-row';
import { SCISSORS_CURSOR } from '../utils';

interface CameraTrackProps {
  segments: CameraSegment[];
  totalDuration: number;
  selectedId: string | null;
  isCutToolActive: boolean;
  onSelect: (id: string | null) => void;
  onResize: (id: string, startTime: number, endTime: number) => void;
  onMove: (id: string, startTime: number, endTime: number) => void;
  onGestureEnd?: () => void;
  onAdd: (startTime: number, endTime: number) => void;
  onCut: (cutTime: number) => void;
  onCutAll: (cutTime: number) => void;
  onDelete: (id: string) => void;
}

export default function CameraTrack({
  segments,
  totalDuration,
  selectedId,
  isCutToolActive,
  onSelect,
  onResize,
  onMove,
  onGestureEnd,
  onAdd,
  onCut,
  onCutAll,
  onDelete,
}: CameraTrackProps) {
  const [contextSegmentId, setContextSegmentId] = useState<string | null>(null);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    const segmentEl = target.closest('[data-segment]');
    setContextSegmentId(segmentEl?.getAttribute('data-segment') ?? null);
  }, []);

  const handleDelete = useCallback(() => {
    if (contextSegmentId) {
      onDelete(contextSegmentId);
    }
  }, [contextSegmentId, onDelete]);

  const renderLabel = useCallback(
    (_segment: TrackSegment, widthPixels: number) => {
      if (widthPixels < 64) {
        return <Camera className="size-3 shrink-0" />;
      }

      return (
        <span className="flex items-center gap-1.5 truncate px-1">
          <Camera className="size-3 shrink-0" />
          {widthPixels >= 100 && (
            <span className="truncate text-xs">Camera</span>
          )}
        </span>
      );
    },
    []
  );

  return (
    <TrackRow>
      <ContextMenu>
        <ContextMenuTrigger
          onContextMenu={handleContextMenu}
          className="block h-full"
        >
          <Track
            segments={segments}
            totalDuration={totalDuration}
            selectedId={selectedId}
            isToolActive={true}
            colors="pink"
            features={{
              canDraw: true,
              canMove: true,
              emptyText: 'Drag to show camera',
              renderLabel,
              allowTrackClickOnSegments: isCutToolActive,
              toolCursor: isCutToolActive ? SCISSORS_CURSOR : undefined,
            }}
            onSelect={onSelect}
            onResize={onResize}
            onMove={onMove}
            onGestureEnd={onGestureEnd}
            onAdd={onAdd}
            onTrackClick={
              isCutToolActive
                ? (time, shiftKey) => {
                    if (shiftKey) {
                      onCut(time);
                      return;
                    }
                    onCutAll(time);
                  }
                : undefined
            }
          />
        </ContextMenuTrigger>
        <ContextMenuContent className="w-40">
          {contextSegmentId ? (
            <ContextMenuItem onClick={handleDelete}>
              <Trash2 className="mr-2 size-4" />
              Delete
            </ContextMenuItem>
          ) : (
            <ContextMenuItem disabled>
              Right-click a camera segment
            </ContextMenuItem>
          )}
        </ContextMenuContent>
      </ContextMenu>
    </TrackRow>
  );
}
