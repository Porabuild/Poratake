import { useEffect, useLayoutEffect, useState } from 'react';
import type { AppearanceConfig } from '@/types/settings';
import { DEFAULT_SETTINGS } from '@/types/settings';
import { applyAppTheme } from '@/renderer/theme/app-theme';

const APPEARANCE_STORAGE_KEY = 'poratake:appearance';

function getInitialAppearance(): AppearanceConfig {
  try {
    const cached = localStorage.getItem(APPEARANCE_STORAGE_KEY);
    if (!cached) return DEFAULT_SETTINGS.appearance;
    const appearance = JSON.parse(cached) as AppearanceConfig;
    if (!['system', 'light', 'dark'].includes(appearance.mode)) {
      return DEFAULT_SETTINGS.appearance;
    }
    return appearance;
  } catch {
    return DEFAULT_SETTINGS.appearance;
  }
}

export function useAppTheme(): void {
  const [appearance, setAppearance] =
    useState<AppearanceConfig>(getInitialAppearance);

  useEffect(() => {
    void window.ipcRenderer
      .invoke('settings:get-appearance')
      .then((nextAppearance: AppearanceConfig) =>
        setAppearance(nextAppearance)
      );

    const handleUpdate = (
      _event: unknown,
      nextAppearance: AppearanceConfig
    ) => {
      setAppearance(nextAppearance);
    };

    window.ipcRenderer.on('settings:appearance-updated', handleUpdate);
    return () => {
      window.ipcRenderer.off('settings:appearance-updated', handleUpdate);
    };
  }, []);

  useLayoutEffect(() => {
    localStorage.setItem(APPEARANCE_STORAGE_KEY, JSON.stringify(appearance));
    return applyAppTheme(appearance);
  }, [appearance]);
}
