import { nativeTheme } from 'electron';
import { getThemePreset } from '@/types/theme';
import { getConfig } from './index';

function getAccentVariant() {
  const appearance = getConfig().appearance;
  const preset = getThemePreset(appearance.theme);
  const isDark =
    appearance.mode === 'system'
      ? nativeTheme.shouldUseDarkColors
      : appearance.mode === 'dark';

  return isDark ? preset.dark : preset.light;
}

export function getAccentColor(): string {
  return getAccentVariant().accent;
}

export function getAccentForegroundColor(): string {
  return getAccentVariant().accentFg;
}
