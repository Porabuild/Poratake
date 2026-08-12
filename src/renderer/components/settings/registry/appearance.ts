import type { ThemeMode } from '@/types/theme';
import { APP_THEME_OPTIONS } from '@/types/theme';
import type { SettingsItem } from '../settings-registry';

export const APPEARANCE_ITEMS: SettingsItem[] = [
  {
    id: 'appearance.mode',
    category: 'appearance',
    section: 'Theme',
    type: 'select',
    label: 'Appearance',
    description: 'Choose a light, dark, or system-matched appearance',
    keywords: ['appearance', 'light', 'dark', 'system', 'mode'],
    options: [
      { value: 'system', label: 'System' },
      { value: 'light', label: 'Light' },
      { value: 'dark', label: 'Dark' },
    ],
    getValue: s => s.appearance.mode,
    setValue: (s, value) => ({
      appearance: { ...s.appearance, mode: value as ThemeMode },
    }),
  },
  {
    id: 'appearance.theme',
    category: 'appearance',
    section: 'Theme',
    type: 'select',
    label: 'Color theme',
    description: 'Use the same paired color themes as Poracode',
    keywords: ['theme', 'color', 'poracode', 'palette'],
    options: APP_THEME_OPTIONS,
    getValue: s => s.appearance.theme,
    setValue: (s, value) => ({
      appearance: { ...s.appearance, theme: value },
    }),
  },
];
