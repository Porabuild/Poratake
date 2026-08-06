import { useState, useCallback, useMemo } from 'react';
import type { NativeVideoPlayerHandle, Segment } from '../types';
import { getTotalTimelineDuration } from '../utils';

interface UsePlaybackControlProps {
  nativePlayerRef: React.RefObject<NativeVideoPlayerHandle | null>;
  segments: Segment[];
  firstFrameDuration?: number;
}

interface UsePlaybackControlReturn {
  isPlaying: boolean;
  timelinePosition: number;
  previewTimelinePosition: number | null;
  effectiveTimelinePosition: number;
  totalTimelineDuration: number;
  playheadPosition: number;
  handleTimeUpdate: (pos: number) => void;
  handlePlayingChange: (playing: boolean) => void;
  togglePlayPause: () => void;
  seekToTimelinePosition: (tlPos: number) => void;
  setPreviewTimelinePosition: React.Dispatch<
    React.SetStateAction<number | null>
  >;
  setTimelinePosition: React.Dispatch<React.SetStateAction<number>>;
}

export function usePlaybackControl({
  nativePlayerRef,
  segments,
  firstFrameDuration = 0,
}: UsePlaybackControlProps): UsePlaybackControlReturn {
  const [isPlaying, setIsPlaying] = useState(false);
  const [timelinePosition, setTimelinePosition] = useState(0);
  const [previewTimelinePosition, setPreviewTimelinePosition] = useState<
    number | null
  >(null);

  const totalTimelineDuration = useMemo(
    () => firstFrameDuration + getTotalTimelineDuration(segments),
    [segments, firstFrameDuration]
  );

  const handleTimeUpdate = useCallback(
    (pos: number) => {
      setTimelinePosition(pos);

      setPreviewTimelinePosition(prev => {
        if (prev === null) return null;
        return nativePlayerRef.current?.isPlaying() ? null : prev;
      });
    },
    [nativePlayerRef]
  );

  const handlePlayingChange = useCallback((playing: boolean) => {
    setIsPlaying(playing);
  }, []);

  const togglePlayPause = useCallback(() => {
    if (!nativePlayerRef.current) return;

    if (nativePlayerRef.current.isPlaying()) {
      nativePlayerRef.current.pause();
      setIsPlaying(false);
      return;
    }

    const previewPos = previewTimelinePosition;
    if (previewPos !== null) {
      setPreviewTimelinePosition(null);
      nativePlayerRef.current.setPreviewTime(null);
      nativePlayerRef.current.seekTo(previewPos);
      setTimelinePosition(previewPos);
    }

    nativePlayerRef.current.play();
    setIsPlaying(true);
  }, [previewTimelinePosition, nativePlayerRef]);

  const seekToTimelinePosition = useCallback(
    (tlPos: number) => {
      if (!nativePlayerRef.current) return;

      setPreviewTimelinePosition(null);
      nativePlayerRef.current.setPreviewTime(null);

      nativePlayerRef.current.seekTo(tlPos);
      setTimelinePosition(tlPos);
    },
    [nativePlayerRef]
  );

  const effectiveTimelinePosition = previewTimelinePosition ?? timelinePosition;

  const playheadPosition = useMemo(() => {
    if (totalTimelineDuration === 0 || segments.length === 0) return 0;
    return (effectiveTimelinePosition / totalTimelineDuration) * 100;
  }, [totalTimelineDuration, segments.length, effectiveTimelinePosition]);

  return {
    isPlaying,
    timelinePosition,
    previewTimelinePosition,
    effectiveTimelinePosition,
    totalTimelineDuration,
    playheadPosition,
    handleTimeUpdate,
    handlePlayingChange,
    togglePlayPause,
    seekToTimelinePosition,
    setPreviewTimelinePosition,
    setTimelinePosition,
  };
}
