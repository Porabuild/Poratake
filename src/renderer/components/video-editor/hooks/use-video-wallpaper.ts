import { useCallback } from 'react';
import type { AspectRatio } from '@/types/aspect-ratio';
import type { GradientOption } from '@/types/editor';
import type { VideoWallpaperSettings } from '@/types/video-wallpaper';
import type { SliceController } from './use-editor-history';

interface UseVideoWallpaperReturn {
  wallpaper: VideoWallpaperSettings;
  setEnabled: (enabled: boolean) => void;
  setGradient: (gradient: GradientOption | null) => void;
  setBackgroundImage: (image: string | null) => void;
  setPadding: (padding: number) => void;
  setCorners: (corners: number) => void;
  setShadow: (shadow: number) => void;
  setAspectRatio: (aspectRatio: AspectRatio | null) => void;
  setDeviceFrame: (deviceFrame: boolean) => void;
  setWallpaper: (wallpaper: VideoWallpaperSettings) => void;
}

export function useVideoWallpaper(
  slice: SliceController<VideoWallpaperSettings>
): UseVideoWallpaperReturn {
  const { value: wallpaper, set } = slice;

  const setGradient = useCallback(
    (gradient: GradientOption | null) => {
      set(prev => {
        const newPadding = gradient && prev.padding === 0 ? 50 : prev.padding;
        return {
          ...prev,
          enabled: gradient ? true : prev.enabled,
          gradient,
          backgroundImage: null,
          padding: newPadding,
        };
      });
    },
    [set]
  );

  const setBackgroundImage = useCallback(
    (backgroundImage: string | null) => {
      set(prev => {
        const newPadding =
          backgroundImage && prev.padding === 0 ? 50 : prev.padding;
        return {
          ...prev,
          enabled: backgroundImage ? true : prev.enabled,
          backgroundImage,
          gradient: null,
          padding: newPadding,
        };
      });
    },
    [set]
  );

  const setPadding = useCallback(
    (padding: number) => set(prev => ({ ...prev, padding })),
    [set]
  );

  const setCorners = useCallback(
    (corners: number) => set(prev => ({ ...prev, corners })),
    [set]
  );

  const setShadow = useCallback(
    (shadow: number) => set(prev => ({ ...prev, shadow })),
    [set]
  );

  const setAspectRatio = useCallback(
    (aspectRatio: AspectRatio | null) =>
      set(prev => ({ ...prev, aspectRatio })),
    [set]
  );

  const setDeviceFrame = useCallback(
    (deviceFrame: boolean) => set(prev => ({ ...prev, deviceFrame })),
    [set]
  );

  const setWallpaper = useCallback(
    (newWallpaper: VideoWallpaperSettings) => set(newWallpaper),
    [set]
  );

  const setEnabled = useCallback(
    (enabled: boolean) => set(prev => ({ ...prev, enabled })),
    [set]
  );

  return {
    wallpaper,
    setEnabled,
    setGradient,
    setBackgroundImage,
    setPadding,
    setCorners,
    setShadow,
    setAspectRatio,
    setDeviceFrame,
    setWallpaper,
  };
}
