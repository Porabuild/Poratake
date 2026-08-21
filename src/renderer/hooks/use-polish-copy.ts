import { useCallback, useEffect, useRef, useState } from 'react';
import type { SettingsConfig, WallpaperPreset } from '@/types/settings';
import { renderWallpaperComposite } from '@/renderer/utils/wallpaper-render';
import { loadImage } from '@/renderer/utils/image';

interface UsePolishCopyReturn {
  preset: WallpaperPreset | null;
  isPolishing: boolean;
  polish: () => Promise<void>;
}

export function usePolishCopy(enabled: boolean): UsePolishCopyReturn {
  const [preset, setPreset] = useState<WallpaperPreset | null>(null);
  const [isPolishing, setIsPolishing] = useState(false);
  const isPolishPending = useRef(false);

  useEffect(() => {
    if (!enabled) return;

    let cancelled = false;

    window.ipcRenderer
      .invoke('wallpaper:getSettings')
      .then((wallpaper: SettingsConfig['wallpaper'] | null) => {
        if (cancelled || !wallpaper?.defaultPresetId) return;
        setPreset(
          wallpaper.presets?.find(p => p.id === wallpaper.defaultPresetId) ??
            null
        );
      })
      .catch((error: unknown) => {
        console.error('Failed to load default wallpaper preset:', error);
      });

    return () => {
      cancelled = true;
    };
  }, [enabled]);

  const polish = useCallback(async () => {
    if (!preset || isPolishPending.current) return;

    isPolishPending.current = true;
    setIsPolishing(true);

    try {
      const sourceDataUrl = (await window.ipcRenderer.invoke(
        'capture-preview:get-source-image'
      )) as string | null;

      if (!sourceDataUrl) {
        throw new Error('Capture image is unavailable');
      }

      const image = await loadImage(sourceDataUrl);
      const canvas = await renderWallpaperComposite(image, preset);

      await window.ipcRenderer.invoke(
        'capture-preview:copy-composited',
        canvas.toDataURL('image/png')
      );
    } catch (error) {
      console.error('Failed to copy with wallpaper preset:', error);
    } finally {
      isPolishPending.current = false;
      setIsPolishing(false);
    }
  }, [preset]);

  return { preset, isPolishing, polish };
}
