import { useState, useEffect, useCallback, useRef, forwardRef } from 'react';
import { Trash2, Play, Mic, Volume2, Video, MousePointer2 } from 'lucide-react';
import { Button } from '@/renderer/components/ui/button';
import type {
  HistoryItemSummary,
  VideoRecordingFeatures,
} from '@/types/history';
import { formatRelativeTime } from '@/renderer/components/history/utils';

interface HistoryItemProps {
  item: HistoryItemSummary;
  isSelected?: boolean;
  onOpen: (item: HistoryItemSummary) => void;
  onDelete: (id: string) => void;
}

const HistoryItem = forwardRef<HTMLDivElement, HistoryItemProps>(
  function HistoryItem({ item, isSelected = false, onOpen, onDelete }, ref) {
    const [imageSrc, setImageSrc] = useState<string | null>(null);
    const [isHovered, setIsHovered] = useState(false);
    const [loading, setLoading] = useState(true);
    const [isVisible, setIsVisible] = useState(false);
    const [videoFeatures, setVideoFeatures] =
      useState<VideoRecordingFeatures | null>(null);
    const internalRef = useRef<HTMLDivElement | null>(null);

    const isVideo = item.type === 'video';

    const setRefs = useCallback(
      (element: HTMLDivElement | null) => {
        internalRef.current = element;
        if (typeof ref === 'function') {
          ref(element);
        } else if (ref) {
          (ref as React.MutableRefObject<HTMLDivElement | null>).current =
            element;
        }
      },
      [ref]
    );

    useEffect(() => {
      const element = internalRef.current;
      if (!element) return;

      const observer = new IntersectionObserver(
        entries => {
          if (entries[0].isIntersecting) {
            setIsVisible(true);
            observer.disconnect();
          }
        },
        { rootMargin: '50px' }
      );

      observer.observe(element);
      return () => observer.disconnect();
    }, []);

    useEffect(() => {
      if (!isVisible) return;

      const loadThumbnail = async () => {
        try {
          const base64 = (await window.ipcRenderer.invoke(
            'history:getThumbnail',
            item.id
          )) as string | null;

          if (base64) {
            setImageSrc(`data:image/jpeg;base64,${base64}`);
          }
        } catch (error) {
          console.error('Failed to load thumbnail:', error);
        } finally {
          setLoading(false);
        }
      };
      loadThumbnail();
    }, [isVisible, item.id]);

    useEffect(() => {
      if (!isVisible || !isVideo) return;

      const loadVideoFeatures = async () => {
        try {
          const features = (await window.ipcRenderer.invoke(
            'history:getVideoFeatures',
            item.id
          )) as VideoRecordingFeatures;
          setVideoFeatures(features);
        } catch (error) {
          console.error('Failed to load video features:', error);
        }
      };
      loadVideoFeatures();
    }, [isVisible, isVideo, item.id]);

    const handleClick = useCallback(() => {
      onOpen(item);
    }, [item, onOpen]);

    const handleDelete = useCallback(
      (e: React.MouseEvent) => {
        e.stopPropagation();
        onDelete(item.id);
      },
      [item.id, onDelete]
    );

    return (
      <div
        ref={setRefs}
        className={`group bg-secondary hover:bg-muted relative cursor-default overflow-hidden rounded-lg transition-all ${
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
            <div className="bg-muted flex h-full w-full items-center justify-center">
              <div className="border-muted-foreground/30 border-t-muted-foreground h-4 w-4 animate-spin rounded-full border-2" />
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
            <div className="bg-muted text-muted-foreground flex h-full w-full items-center justify-center text-xs">
              No preview
            </div>
          )}
        </div>

        <div className="px-2 py-1.5">
          <p className="text-muted-foreground text-xs">
            {formatRelativeTime(item.timestamp)}
          </p>
        </div>

        {(isHovered || isSelected) && (
          <Button
            variant="ghost"
            size="icon"
            onClick={handleDelete}
            className="absolute top-1 right-1 h-6 w-6 bg-black/60 text-white hover:bg-red-500/80 hover:text-white"
          >
            <Trash2 className="h-3 w-3" />
          </Button>
        )}
      </div>
    );
  }
);

export default HistoryItem;
