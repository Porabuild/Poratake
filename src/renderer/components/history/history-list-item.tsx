import { forwardRef, useCallback } from 'react';
import {
  Trash2,
  Play,
  Camera,
  Mic,
  Volume2,
  Video,
  MousePointer2,
  FolderOpen,
} from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import { formatRelativeTime } from '@/renderer/components/history/utils';
import { useHistoryItem } from './use-history-item';
import type { HistoryItemProps } from './use-history-item';

const HistoryListItem = forwardRef<HTMLDivElement, HistoryItemProps>(
  function HistoryListItem(
    { item, isSelected = false, onOpen, onDelete },
    ref
  ) {
    const {
      elementRef,
      imageSrc,
      loading,
      isHovered,
      setIsHovered,
      videoFeatures,
      isVideo,
      handleClick,
      handleDelete,
      handleReveal,
    } = useHistoryItem({ item, onOpen, onDelete });

    const setRefs = useCallback(
      (element: HTMLDivElement | null) => {
        elementRef.current = element;
        if (typeof ref === 'function') {
          ref(element);
        } else if (ref) {
          ref.current = element;
        }
      },
      [ref, elementRef]
    );

    return (
      <div
        ref={setRefs}
        className={`group flex cursor-default items-center gap-3 rounded-lg bg-secondary p-2 transition-all hover:bg-muted ${
          isSelected
            ? 'ring-2 ring-primary ring-offset-1 ring-offset-transparent'
            : ''
        }`}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
        onClick={handleClick}
      >
        <div className="relative h-12 w-20 shrink-0 overflow-hidden rounded-md">
          {loading ? (
            <div className="flex h-full w-full items-center justify-center bg-muted">
              <div className="h-3 w-3 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground" />
            </div>
          ) : imageSrc ? (
            <>
              <img
                src={imageSrc}
                alt={isVideo ? 'Video' : 'Screenshot'}
                className="h-full w-full object-cover"
              />
              {isVideo && (
                <div className="absolute inset-0 flex items-center justify-center">
                  <div className="flex h-6 w-6 items-center justify-center rounded-full bg-black/60 text-white">
                    <Play className="h-3 w-3 fill-current" />
                  </div>
                </div>
              )}
            </>
          ) : (
            <div className="flex h-full w-full items-center justify-center bg-muted text-xs text-muted-foreground">
              No preview
            </div>
          )}
        </div>

        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <div className="flex items-center gap-1.5">
            {isVideo ? (
              <Video className="h-3 w-3 shrink-0 text-muted-foreground" />
            ) : (
              <Camera className="h-3 w-3 shrink-0 text-muted-foreground" />
            )}
            <span className="truncate text-xs font-medium text-foreground">
              {isVideo ? 'Video' : 'Screenshot'}
            </span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="text-xs text-muted-foreground">
              {formatRelativeTime(item.timestamp)}
            </span>
            {isVideo && videoFeatures && (
              <div className="flex gap-0.5">
                {videoFeatures.hasMic && (
                  <Mic className="h-2.5 w-2.5 text-muted-foreground" />
                )}
                {videoFeatures.hasSystemAudio && (
                  <Volume2 className="h-2.5 w-2.5 text-muted-foreground" />
                )}
                {videoFeatures.hasCamera && (
                  <Video className="h-2.5 w-2.5 text-muted-foreground" />
                )}
                {videoFeatures.hasCursor && (
                  <MousePointer2 className="h-2.5 w-2.5 text-muted-foreground" />
                )}
              </div>
            )}
          </div>
        </div>

        {(isHovered || isSelected) && (
          <>
            <Button
              variant="ghost"
              size="icon"
              onClick={handleReveal}
              title="Show in folder"
              aria-label="Show in folder"
              className="h-6 w-6 shrink-0 text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
            >
              <FolderOpen className="h-3 w-3" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              onClick={handleDelete}
              className="h-6 w-6 shrink-0 text-red-400 hover:bg-red-500/20 hover:text-red-400"
            >
              <Trash2 className="h-3 w-3" />
            </Button>
          </>
        )}
      </div>
    );
  }
);

export default HistoryListItem;
