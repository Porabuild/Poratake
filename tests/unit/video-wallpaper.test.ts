// @vitest-environment happy-dom
import {
  act,
  createElement,
  createRef,
  forwardRef,
  useImperativeHandle,
} from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import { useVideoWallpaper } from '../../src/renderer/components/video-editor/hooks/use-video-wallpaper';
import type { SliceController } from '../../src/renderer/components/video-editor/hooks/use-editor-history';
import {
  DEFAULT_VIDEO_WALLPAPER,
  type VideoWallpaperSettings,
} from '../../src/types/video-wallpaper';

interface WallpaperHarnessProps {
  slice: SliceController<VideoWallpaperSettings>;
}

const WallpaperHarness = forwardRef<
  ReturnType<typeof useVideoWallpaper>,
  WallpaperHarnessProps
>(function WallpaperHarness({ slice }, ref) {
  const controller = useVideoWallpaper(slice);
  useImperativeHandle(ref, () => controller, [controller]);
  return null;
});

const mountedRoots: Array<{ container: HTMLDivElement; root: Root }> = [];

afterEach(() => {
  for (const { container, root } of mountedRoots.splice(0)) {
    act(() => root.unmount());
    container.remove();
  }
});

function createWallpaperController(): {
  controller: ReturnType<typeof useVideoWallpaper>;
  getWallpaper: () => VideoWallpaperSettings;
} {
  let wallpaper = DEFAULT_VIDEO_WALLPAPER;

  const set: SliceController<VideoWallpaperSettings>['set'] = updater => {
    wallpaper = typeof updater === 'function' ? updater(wallpaper) : updater;
  };

  const slice: SliceController<VideoWallpaperSettings> = {
    value: wallpaper,
    set,
    setWithoutHistory: set,
    commit: () => {},
  };

  const container = document.createElement('div');
  const root = createRoot(container);
  const controllerRef = createRef<ReturnType<typeof useVideoWallpaper>>();
  mountedRoots.push({ container, root });
  act(() => {
    root.render(createElement(WallpaperHarness, { ref: controllerRef, slice }));
  });

  if (!controllerRef.current) {
    throw new Error('Wallpaper controller was not initialized');
  }

  return {
    controller: controllerRef.current,
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
