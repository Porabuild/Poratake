import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { useVideoWallpaper } from '../../src/renderer/components/video-editor/hooks/use-video-wallpaper';
import type { SliceController } from '../../src/renderer/components/video-editor/hooks/use-editor-history';
import {
  DEFAULT_VIDEO_WALLPAPER,
  type VideoWallpaperSettings,
} from '../../src/types/video-wallpaper';

function createWallpaperController(): {
  controller: ReturnType<typeof useVideoWallpaper>;
  getWallpaper: () => VideoWallpaperSettings;
} {
  let wallpaper = DEFAULT_VIDEO_WALLPAPER;
  let controller: ReturnType<typeof useVideoWallpaper> | null = null;

  const set: SliceController<VideoWallpaperSettings>['set'] = updater => {
    wallpaper = typeof updater === 'function' ? updater(wallpaper) : updater;
  };

  const slice: SliceController<VideoWallpaperSettings> = {
    value: wallpaper,
    set,
    setWithoutHistory: set,
    commit: () => {},
  };

  function Harness() {
    controller = useVideoWallpaper(slice);
    return null;
  }

  renderToStaticMarkup(createElement(Harness));

  if (!controller) {
    throw new Error('Wallpaper controller was not initialized');
  }

  return {
    controller,
    getWallpaper: () => wallpaper,
  };
}

describe('useVideoWallpaper', () => {
  it('enables wallpaper in the same update that selects an image', () => {
    const { controller, getWallpaper } = createWallpaperController();

    controller.setBackgroundImage('data:image/png;base64,image');

    expect(getWallpaper()).toMatchObject({
      enabled: true,
      backgroundImage: 'data:image/png;base64,image',
      gradient: null,
      padding: 50,
    });
  });

  it('enables wallpaper in the same update that selects a gradient', () => {
    const { controller, getWallpaper } = createWallpaperController();
    const gradient = {
      id: 'gradient',
      colors: ['#000000', '#ffffff'],
      angle: 90,
    };

    controller.setGradient(gradient);

    expect(getWallpaper()).toMatchObject({
      enabled: true,
      backgroundImage: null,
      gradient,
      padding: 50,
    });
  });
});
