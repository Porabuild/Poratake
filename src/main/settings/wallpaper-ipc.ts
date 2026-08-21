import { dialog, ipcMain } from 'electron';
import fs from 'fs';
import { pathToFileURL } from 'url';
import { daemon } from '@/main/daemon';
import type { CustomBackground, WallpaperPreset } from '@/types/settings.ts';
import {
  persistCustomBackground,
  persistWallpaperPreset,
} from './wallpaper-assets.ts';
import { getConfig, updateConfig } from './store.ts';

export function registerWallpaperIpc(): void {
  ipcMain.handle('wallpaper:getSettings', () => {
    return getConfig().wallpaper;
  });

  ipcMain.handle(
    'wallpaper:addBackground',
    (_event, background: CustomBackground) => {
      const persistedBackground = persistCustomBackground(background);
      const wallpaper = {
        ...getConfig().wallpaper,
        customBackgrounds: [
          ...getConfig().wallpaper.customBackgrounds,
          persistedBackground,
        ],
      };
      updateConfig({ wallpaper });
      return wallpaper.customBackgrounds;
    }
  );

  ipcMain.handle(
    'wallpaper:updateBackground',
    (_event, background: CustomBackground) => {
      const wallpaperConfig = getConfig().wallpaper;
      if (
        !wallpaperConfig.customBackgrounds.some(
          item => item.id === background.id
        )
      ) {
        return wallpaperConfig.customBackgrounds;
      }

      const persistedBackground = persistCustomBackground(background);
      const wallpaper = {
        ...wallpaperConfig,
        customBackgrounds: wallpaperConfig.customBackgrounds.map(item =>
          item.id === background.id ? persistedBackground : item
        ),
      };
      updateConfig({ wallpaper });
      return wallpaper.customBackgrounds;
    }
  );

  ipcMain.handle('wallpaper:deleteBackground', (_event, id: string) => {
    const wallpaperConfig = getConfig().wallpaper;
    const wallpaper = {
      ...wallpaperConfig,
      customBackgrounds: wallpaperConfig.customBackgrounds.filter(
        background => background.id !== id
      ),
    };
    updateConfig({ wallpaper });
    return wallpaper.customBackgrounds;
  });

  ipcMain.handle('wallpaper:addPreset', (_event, preset: WallpaperPreset) => {
    const persistedPreset = persistWallpaperPreset(preset);
    const wallpaper = {
      ...getConfig().wallpaper,
      presets: [...getConfig().wallpaper.presets, persistedPreset],
    };
    updateConfig({ wallpaper });
    return wallpaper.presets;
  });

  ipcMain.handle(
    'wallpaper:updatePreset',
    (_event, preset: WallpaperPreset) => {
      const wallpaperConfig = getConfig().wallpaper;
      if (!wallpaperConfig.presets.some(item => item.id === preset.id)) {
        return wallpaperConfig.presets;
      }

      const persistedPreset = persistWallpaperPreset(preset);
      const wallpaper = {
        ...wallpaperConfig,
        presets: wallpaperConfig.presets.map(item =>
          item.id === preset.id ? persistedPreset : item
        ),
      };
      updateConfig({ wallpaper });
      return wallpaper.presets;
    }
  );

  ipcMain.handle('wallpaper:deletePreset', (_event, id: string) => {
    const wallpaperConfig = getConfig().wallpaper;
    const wallpaper = {
      ...wallpaperConfig,
      presets: wallpaperConfig.presets.filter(preset => preset.id !== id),
      defaultPresetId:
        wallpaperConfig.defaultPresetId === id
          ? null
          : wallpaperConfig.defaultPresetId,
    };
    updateConfig({ wallpaper });
    return wallpaper.presets;
  });

  ipcMain.handle('wallpaper:setDefaultPreset', (_event, id: string | null) => {
    const wallpaperConfig = getConfig().wallpaper;
    const wallpaper = {
      ...wallpaperConfig,
      defaultPresetId:
        id && wallpaperConfig.presets.some(preset => preset.id === id)
          ? id
          : null,
    };
    updateConfig({ wallpaper });
    return wallpaper.defaultPresetId;
  });

  ipcMain.handle('wallpaper:selectImage', async () => {
    try {
      const result = await dialog.showOpenDialog({
        properties: ['openFile'],
        filters: [
          {
            name: 'Images',
            extensions: [
              'png',
              'jpg',
              'jpeg',
              'jfif',
              'svg',
              'webp',
              'gif',
              'bmp',
            ],
          },
        ],
      });

      if (result.canceled || result.filePaths.length === 0) {
        return null;
      }

      return pathToFileURL(result.filePaths[0]).href;
    } catch (error) {
      console.error('Failed to select image:', error);
      return null;
    }
  });

  ipcMain.handle('wallpaper:getDesktopWallpaper', async () => {
    try {
      const result = await daemon.call<{ type: string; value: string }>(
        'desktop-wallpaper',
        'get'
      );

      if (!result) {
        return null;
      }

      if (result.type === 'data') {
        return result.value;
      }

      if (result.type === 'path') {
        const filePath = result.value;
        if (!fs.existsSync(filePath)) {
          console.error('Desktop wallpaper file not found:', filePath);
          return null;
        }

        return pathToFileURL(filePath).href;
      }

      return null;
    } catch (error) {
      console.error('Failed to get desktop wallpaper:', error);
      return null;
    }
  });
}
