import { nativeTheme } from 'electron';
import type { BrowserWindow, BrowserWindowConstructorOptions } from 'electron';
import { release } from 'node:os';
import type { AppearanceConfig } from '@/types/settings';
import { getThemePreset } from '@/types/theme';
import { isMac, isWindows } from './platform';

export const TITLE_BAR_HEIGHT = 40;

export type TitleBarSurface = 'background' | 'card';

export interface TitleBarOptions {
  height?: number;
  surface?: TitleBarSurface;
  syncBackground?: boolean;
  transparent?: boolean;
  trafficLightPosition?: { x: number; y: number };
}

let activeTheme = 'default';
const themeListeners = new Set<() => void>();

export function supportsWindowsAcrylic(osRelease = release()): boolean {
  if (!isWindows) return false;
  const build = Number(osRelease.split('.')[2] ?? '0');
  return Number.isFinite(build) && build >= 22621;
}

export function supportsNativeWindowMaterial(osRelease = release()): boolean {
  return isMac || supportsWindowsAcrylic(osRelease);
}

export function nativeWindowMaterialOptions(
  osRelease = release()
): BrowserWindowConstructorOptions {
  if (isMac) {
    return {
      vibrancy: 'sidebar',
      visualEffectState: 'active',
      transparent: true,
    };
  }

  if (supportsWindowsAcrylic(osRelease)) {
    return { backgroundMaterial: 'acrylic' };
  }

  return {};
}

export function applyTitleBarAppearance(appearance: AppearanceConfig): void {
  activeTheme = appearance.theme;
  nativeTheme.themeSource = appearance.mode;
  themeListeners.forEach(listener => listener());
}

export function titleBarColors(surface: TitleBarSurface = 'card'): {
  color: string;
  symbolColor: string;
} {
  const theme = nativeTheme.shouldUseDarkColors ? 'dark' : 'light';
  const variant = getThemePreset(activeTheme)[theme];

  return {
    color: surface === 'card' ? variant.surface : variant.bg,
    symbolColor: variant.fg,
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
      ...(options.transparent && { color: '#00000000' }),
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
      ...(options.transparent && { color: '#00000000' }),
      height: options.height ?? TITLE_BAR_HEIGHT,
    });

    if (options.syncBackground) {
      window.setBackgroundColor(colors.color);
    }
  };

  window.once('ready-to-show', apply);
  themeListeners.add(apply);
  nativeTheme.on('updated', apply);
  window.once('closed', () => {
    themeListeners.delete(apply);
    nativeTheme.off('updated', apply);
  });
}
