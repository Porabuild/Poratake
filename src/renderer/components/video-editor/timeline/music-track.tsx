import { useCallback, useMemo } from 'react';
import { Trash2, Gauge } from 'lucide-react';
import {
  PLAYBACK_SPEED_PRESETS,
  formatPlaybackSpeed,
} from '@/types/playback-speed';
import type { MusicTrack as MusicTrackType } from '@/types/music';
import { SOURCE_ICONS } from '@/types/music';
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from '@/renderer/components/ui/context-menu';
import Track, { type TrackSegment } from './track';
import TrackRow from './track-row';
import type { TrackColors } from './track-colors';
import { TRACK_COLORS } from './track-colors';
import { formatDuration, SCISSORS_CURSOR } from '../utils';

interface MusicTrackProps {
  tracks: MusicTrackType[];
  totalDuration: number;
  selectedId: string | null;
  isCutToolActive: boolean;
  onSelect: (id: string | null) => void;
  onResize: (id: string, startTime: number, endTime: number) => void;
  onMove: (id: string, startTime: number, endTime: number) => void;
  onGestureEnd?: () => void;
  onSpeedChange: (groupId: string, speed: number) => void;
  onCut: (id: string, cutTime: number) => void;
  onCutAll: (cutTime: number) => void;
  onDelete: (groupId: string) => void;
}

const DISABLED_COLORS: TrackColors = {
  border: 'border-neutral-600',
  gradient: ['#525252', '#404040'],
  selectedGradient: ['#525252', '#404040'],
};

export default function MusicTrack({
  tracks,
  totalDuration,
  selectedId,
  isCutToolActive,
  onSelect,
  onResize,
  onMove,
  onGestureEnd,
  onSpeedChange,
  onCut,
  onCutAll,
  onDelete,
}: MusicTrackProps) {
  const group = tracks[0];

  const segments: TrackSegment[] = useMemo(
    () =>
      tracks.map(track => ({
        id: track.id,
        startTime: track.startTime,
        endTime: track.endTime,
      })),
    [tracks]
  );

  const trackById = useMemo(
    () => new Map(tracks.map(track => [track.id, track])),
    [tracks]
  );

  const Icon = SOURCE_ICONS[group.source];
  const colors = group.enabled ? TRACK_COLORS.purple : DISABLED_COLORS;

  const renderLabel = useCallback(
    (segment: TrackSegment, widthPixels: number) => {
      const track = trackById.get(segment.id);
      if (!track) return null;

      const hasSpeedChange = track.speed !== 1;
      const duration = track.endTime - track.startTime;

      if (widthPixels < 60) {
        return <Icon className="size-3 shrink-0 text-white" />;
      }

      return (
        <span className="inline-flex items-center gap-1.5 truncate px-1">
          <Icon className="size-2.5 shrink-0" />
          {widthPixels >= 100 && (
            <span className="truncate text-xs">{track.name}</span>
          )}
          {widthPixels >= 140 && (
            <span className="text-xs opacity-70">
              {formatDuration(duration)}
            </span>
          )}
          {hasSpeedChange && widthPixels >= 100 && (
            <span className="rounded bg-white/20 px-1 py-0.5 text-xs font-medium">
              {formatPlaybackSpeed(track.speed)}
            </span>
          )}
        </span>
      );
    },
    [trackById, Icon]
  );

  const handleDelete = useCallback(() => {
    onDelete(group.groupId);
  }, [onDelete, group.groupId]);

  const handleSpeedChange = useCallback(
    (speed: number) => {
      onSpeedChange(group.groupId, speed);
    },
    [onSpeedChange, group.groupId]
  );

  const isRemovable = group.source === 'music';

  return (
    <TrackRow>
      <ContextMenu>
        <ContextMenuTrigger className="block h-full">
          <Track
            segments={segments}
            totalDuration={totalDuration}
            selectedId={selectedId}
            isToolActive={true}
            colors={colors}
            features={{
              canMove: true,
              renderLabel,
              allowTrackClickOnSegments: isCutToolActive,
              toolCursor: isCutToolActive ? SCISSORS_CURSOR : undefined,
            }}
            onSelect={onSelect}
            onResize={onResize}
            onMove={onMove}
            onGestureEnd={onGestureEnd}
            onTrackClick={
              isCutToolActive
                ? (time, shiftKey) => {
                    if (!shiftKey) {
                      onCutAll(time);
                      return;
                    }
                    const target = tracks.find(
                      track => time >= track.startTime && time <= track.endTime
                    );
                    if (target) onCut(target.id, time);
                  }
                : undefined
            }
          />
        </ContextMenuTrigger>
        <ContextMenuContent className="w-48">
          <ContextMenuSub>
            <ContextMenuSubTrigger>
              <Gauge className="mr-2 size-4" />
              Speed
            </ContextMenuSubTrigger>
            <ContextMenuSubContent className="w-32">
              {PLAYBACK_SPEED_PRESETS.map(speed => (
                <ContextMenuItem
                  key={speed}
                  onClick={() => handleSpeedChange(speed)}
                  className={group.speed === speed ? 'bg-accent' : ''}
                >
                  {formatPlaybackSpeed(speed)}
                  {group.speed === speed && (
                    <span className="ml-auto text-xs">*</span>
                  )}
                </ContextMenuItem>
              ))}
            </ContextMenuSubContent>
          </ContextMenuSub>
          {isRemovable && (
            <>
              <ContextMenuSeparator />
              <ContextMenuItem onClick={handleDelete}>
                <Trash2 className="mr-2 size-4" />
                Remove
              </ContextMenuItem>
            </>
          )}
        </ContextMenuContent>
      </ContextMenu>
    </TrackRow>
  );
}
