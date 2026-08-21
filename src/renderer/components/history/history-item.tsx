import { forwardRef, useCallback } from 'react';
import {
  Trash2,
  Play,
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

const HistoryItem = forwardRef<HTMLDivElement, HistoryItemProps>(
  function HistoryItem({ item, isSelected = false, onOpen, onDelete }, ref) {
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
        className={`group relative cursor-default overflow-hidden rounded-lg bg-secondary transition-all hover:bg-muted ${
          isSelected
            ? 'ring-2 ring-blue-500 ring-offset-1 ring-offset-transparent'
            : ''
        }`}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
        onClick={handleClick}
      >
        <div className="relative aspect-video w-full overflow-hidden">
          {loading ? (
            <div className="flex h-full w-full items-center justify-center bg-muted">
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground" />
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
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-black/60 text-white">
                    <Play className="h-5 w-5 fill-current" />
                  </div>
                </div>
              )}
              {isVideo && videoFeatures && (
                <div className="absolute bottom-1 left-1 flex gap-0.5">
                  {videoFeatures.hasMic && (
                    <div className="flex h-4 w-4 items-center justify-center rounded bg-black/60 text-white">
                      <Mic className="h-2.5 w-2.5" />
                    </div>
                  )}
                  {videoFeatures.hasSystemAudio && (
                    <div className="flex h-4 w-4 items-center justify-center rounded bg-black/60 text-white">
                      <Volume2 className="h-2.5 w-2.5" />
                    </div>
                  )}
                  {videoFeatures.hasCamera && (
                    <div className="flex h-4 w-4 items-center justify-center rounded bg-black/60 text-white">
                      <Video className="h-2.5 w-2.5" />
                    </div>
                  )}
                  {videoFeatures.hasCursor && (
                    <div className="flex h-4 w-4 items-center justify-center rounded bg-black/60 text-white">
                      <MousePointer2 className="h-2.5 w-2.5" />
                    </div>
                  )}
                </div>
              )}
            </>
          ) : (
            <div className="flex h-full w-full items-center justify-center bg-muted text-xs text-muted-foreground">
              No preview
            </div>
          )}
        </div>

        <div className="px-2 py-1.5">
          <p className="text-xs text-muted-foreground">
            {formatRelativeTime(item.timestamp)}
          </p>
        </div>

        {(isHovered || isSelected) && (
          <div className="absolute top-1 right-1 flex gap-1">
            <Button
              variant="ghost"
              size="icon"
              onClick={handleReveal}
              title="Show in folder"
              aria-label="Show in folder"
              className="h-6 w-6 bg-black/60 text-white transition-colors hover:bg-black/80 hover:text-white"
            >
              <FolderOpen className="h-3 w-3" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              onClick={handleDelete}
              className="h-6 w-6 bg-black/60 text-white hover:bg-red-500/80 hover:text-white"
            >
              <Trash2 className="h-3 w-3" />
            </Button>
          </div>
        )}
      </div>
    );
  }
);

export default HistoryItem;
