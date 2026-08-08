import { nativeTheme } from 'electron';
import type { BrowserWindow, BrowserWindowConstructorOptions } from 'electron';
import { isMac } from './platform';

export const TITLE_BAR_HEIGHT = 40;

export type TitleBarSurface = 'background' | 'card';

export interface TitleBarOptions {
  height?: number;
  surface?: TitleBarSurface;
  syncBackground?: boolean;
  trafficLightPosition?: { x: number; y: number };
}

const SURFACE_COLORS: Record<TitleBarSurface, { dark: string; light: string }> =
  {
    background: { dark: '#181818', light: '#ffffff' },
    card: { dark: '#1f1f1f', light: '#ffffff' },
  };

const SYMBOL_COLORS = { dark: '#f8f8f8', light: '#000000' };

export function titleBarColors(surface: TitleBarSurface = 'card'): {
  color: string;
  symbolColor: string;
} {
  const theme = nativeTheme.shouldUseDarkColors ? 'dark' : 'light';

  return {
    color: SURFACE_COLORS[surface][theme],
    symbolColor: SYMBOL_COLORS[theme],
  };
}

export function titleBarWindowOptions(
  options: TitleBarOptions = {}
): BrowserWindowConstructorOptions {
  if (isMac) {
    return {
      titleBarStyle: 'hiddenInset',
      ...(options.trafficLightPosition && {
        trafficLightPosition: options.trafficLightPosition,
      }),
    };
  }

  return {
    titleBarStyle: 'hidden',
    titleBarOverlay: {
      ...titleBarColors(options.surface),
      height: options.height ?? TITLE_BAR_HEIGHT,
    },
  };
}

export function trackTitleBarTheme(
  window: BrowserWindow,
  options: TitleBarOptions = {}
): void {
  if (isMac) return;

  const apply = () => {
    if (window.isDestroyed()) return;

    const colors = titleBarColors(options.surface);
    window.setTitleBarOverlay({
      ...colors,
      height: options.height ?? TITLE_BAR_HEIGHT,
    });

    if (options.syncBackground) {
      window.setBackgroundColor(colors.color);
    }
  };

  window.once('ready-to-show', apply);
  nativeTheme.on('updated', apply);
  window.once('closed', () => {
    nativeTheme.off('updated', apply);
  });
}
