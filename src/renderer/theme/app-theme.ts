import type { AppearanceConfig } from '@/types/settings';
import type { ThemeMode, ThemeVariant } from '@/types/theme';
import { getThemePreset } from '@/types/theme';

function resolveMode(mode: ThemeMode): 'light' | 'dark' {
  if (mode !== 'system') return mode;
  return window.matchMedia('(prefers-color-scheme: dark)').matches
    ? 'dark'
    : 'light';
}

function mix(color: string, amount: number, target: string): string {
  return `color-mix(in oklab, ${color} ${amount}%, ${target})`;
}

function applyVariant(
  root: HTMLElement,
  variant: ThemeVariant,
  mode: 'light' | 'dark'
): void {
  const content = variant.content ?? mix(variant.bg, 84, variant.surface);
  const fieldBorder =
    mode === 'dark'
      ? mix(variant.border, 84, variant.fg)
      : mix(variant.border, 72, variant.surface);
  const border =
    mode === 'dark'
      ? mix(variant.border, 90, variant.fg)
      : mix(variant.border, 72, variant.surface);
  const defaultSurface =
    mode === 'dark'
      ? mix(variant.surface, 91, variant.fg)
      : mix(variant.surface, 86, variant.fg);
  // Dark fields sit on the button surface so they read as button-like rather
  // than as black holes; light fields stay near the surface tone.
  const fieldBackground =
    mode === 'dark' ? defaultSurface : mix(variant.surface, 97, variant.bg);
  const accentHoverTarget =
    variant.accentFg.toLowerCase() === '#ffffff' ? '#000000' : '#ffffff';
  const variables: Record<string, string> = {
    '--background': variant.bg,
    '--foreground': variant.fg,
    '--surface': variant.surface,
    '--surface-secondary': mix(variant.surface, 88, variant.bg),
    '--surface-tertiary': mix(variant.surface, 74, variant.bg),
    '--overlay': mix(variant.surface, 94, variant.bg),
    '--muted': mix(variant.fg, 76, variant.bg),
    '--scrollbar': mix(variant.fg, 24, variant.bg),
    '--default': defaultSurface,
    '--accent': variant.accent,
    '--accent-foreground': variant.accentFg,
    '--accent-hover': mix(variant.accent, 90, accentHoverTarget),
    '--field-background': fieldBackground,
    '--field-foreground': variant.fg,
    '--field-placeholder': mix(
      variant.fg,
      mode === 'dark' ? 82 : 73,
      variant.bg
    ),
    '--field-border': fieldBorder,
    '--segment': mix(variant.surface, 82, variant.bg),
    '--border': border,
    '--separator': mix(border, 85, 'transparent'),
    '--sidebar-background': variant.sidebar,
    '--content-background': content,
    '--row-hover': mix(variant.fg, 6, 'transparent'),
    '--row-active': mix(variant.fg, 11, 'transparent'),
    '--hairline': mix(variant.fg, 9, 'transparent'),
    '--card': variant.surface,
    '--card-foreground': variant.fg,
    '--popover': variant.surface,
    '--popover-foreground': variant.fg,
    '--primary': variant.accent,
    '--primary-foreground': variant.accentFg,
    '--secondary': defaultSurface,
    '--secondary-foreground': variant.fg,
    '--muted-background': mix(variant.surface, 88, variant.bg),
    '--muted-foreground': mix(variant.fg, 76, variant.bg),
    '--input': fieldBorder,
    '--ring': mix(variant.accent, 52, 'transparent'),
  };

  Object.entries(variables).forEach(([name, value]) => {
    root.style.setProperty(name, value);
  });
}

export function applyAppTheme(appearance: AppearanceConfig): () => void {
  const media = window.matchMedia('(prefers-color-scheme: dark)');

  const apply = () => {
    const mode = resolveMode(appearance.mode);
    const root = document.documentElement;
    const preset = getThemePreset(appearance.theme);
    root.classList.toggle('dark', mode === 'dark');
    root.classList.toggle('light', mode === 'light');
    root.dataset.theme = mode;
    root.dataset.themePreset = preset.id;
    root.style.colorScheme = mode;
    applyVariant(root, preset[mode], mode);
  };

  apply();
  if (appearance.mode === 'system') media.addEventListener('change', apply);

  return () => media.removeEventListener('change', apply);
}
