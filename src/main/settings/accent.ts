import { nativeTheme } from 'electron';
import { getThemePreset } from '@/types/theme';
import { getConfig } from './index';

export function getAccentColor(): string {
  const appearance = getConfig().appearance;
  const preset = getThemePreset(appearance.theme);
  const isDark =
    appearance.mode === 'system'
      ? nativeTheme.shouldUseDarkColors
      : appearance.mode === 'dark';

  return isDark ? preset.dark.accent : preset.light.accent;
}
