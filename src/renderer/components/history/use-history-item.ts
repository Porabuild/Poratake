import { useCallback, useEffect, useRef, useState } from 'react';
import type { MouseEvent, RefObject } from 'react';
import type {
  HistoryItemSummary,
  VideoRecordingFeatures,
} from '@/types/history';

const VISIBILITY_ROOT_MARGIN = '50px';

export interface HistoryItemProps {
  item: HistoryItemSummary;
  isSelected?: boolean;
  onOpen: (item: HistoryItemSummary) => void;
  onDelete: (id: string) => void;
}

export function useHistoryItem({ item, onOpen, onDelete }: HistoryItemProps) {
  const [imageSrc, setImageSrc] = useState<string | null>(null);
  const [isHovered, setIsHovered] = useState(false);
  const [loading, setLoading] = useState(true);
  const [isVisible, setIsVisible] = useState(false);
  const [videoFeatures, setVideoFeatures] =
    useState<VideoRecordingFeatures | null>(null);
  const internalRef = useRef<HTMLDivElement | null>(null);

  const isVideo = item.type === 'video';

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
      { rootMargin: VISIBILITY_ROOT_MARGIN }
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
    (e: MouseEvent) => {
      e.stopPropagation();
      onDelete(item.id);
    },
    [item.id, onDelete]
  );

  const handleReveal = useCallback(
    (e: MouseEvent) => {
      e.stopPropagation();
      window.ipcRenderer.invoke('history:reveal', item.id);
    },
    [item.id]
  );

  return {
    elementRef: internalRef as RefObject<HTMLDivElement | null>,
    imageSrc,
    loading,
    isHovered,
    setIsHovered,
    videoFeatures,
    isVideo,
    handleClick,
    handleDelete,
    handleReveal,
  };
}
