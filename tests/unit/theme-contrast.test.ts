import { afterEach, describe, expect, it, vi } from 'vitest';

import { applyAppTheme } from '@/renderer/theme/app-theme';
import { APP_THEME_PRESETS } from '@/types/theme';

interface Color {
  r: number;
  g: number;
  b: number;
}

interface Oklab {
  l: number;
  a: number;
  b: number;
}

function parseHex(value: string): Color {
  return {
    r: Number.parseInt(value.slice(1, 3), 16) / 255,
    g: Number.parseInt(value.slice(3, 5), 16) / 255,
    b: Number.parseInt(value.slice(5, 7), 16) / 255,
  };
}

function toLinear(value: number): number {
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}

function toSrgb(value: number): number {
  const result =
    value <= 0.0031308 ? value * 12.92 : 1.055 * value ** (1 / 2.4) - 0.055;
  return Math.min(1, Math.max(0, result));
}

function toOklab(color: Color): Oklab {
  const r = toLinear(color.r);
  const g = toLinear(color.g);
  const b = toLinear(color.b);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return {
    l: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}

function fromOklab(color: Oklab): Color {
  const l = (color.l + 0.3963377774 * color.a + 0.2158037573 * color.b) ** 3;
  const m = (color.l - 0.1055613458 * color.a - 0.0638541728 * color.b) ** 3;
  const s = (color.l - 0.0894841775 * color.a - 1.291485548 * color.b) ** 3;
  return {
    r: toSrgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    g: toSrgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    b: toSrgb(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
  };
}

function resolveColor(value: string): Color {
  if (value.startsWith('#')) return parseHex(value);
  const match = value.match(
    /^color-mix\(in oklab, (#[0-9a-f]{6}) (\d+)%, (#[0-9a-f]{6})\)$/i
  );
  if (!match) throw new Error(`Unsupported color: ${value}`);
  const amount = Number(match[2]) / 100;
  const first = toOklab(parseHex(match[1]));
  const second = toOklab(parseHex(match[3]));
  return fromOklab({
    l: first.l * amount + second.l * (1 - amount),
    a: first.a * amount + second.a * (1 - amount),
    b: first.b * amount + second.b * (1 - amount),
  });
}

function luminance(color: Color): number {
  return (
    0.2126 * toLinear(color.r) +
    0.7152 * toLinear(color.g) +
    0.0722 * toLinear(color.b)
  );
}

function contrast(first: string, second: string): number {
  const firstLuminance = luminance(resolveColor(first));
  const secondLuminance = luminance(resolveColor(second));
  const lighter = Math.max(firstLuminance, secondLuminance);
  const darker = Math.min(firstLuminance, secondLuminance);
  return (lighter + 0.05) / (darker + 0.05);
}

function applyTheme(
  theme: string,
  mode: 'light' | 'dark'
): Map<string, string> {
  const values = new Map<string, string>();
  const root = {
    classList: { toggle: vi.fn() },
    dataset: {} as Record<string, string>,
    style: {
      colorScheme: '',
      setProperty: (name: string, value: string) => values.set(name, value),
    },
  };
  const media = {
    matches: mode === 'dark',
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal('document', { documentElement: root });
  vi.stubGlobal('window', { matchMedia: () => media });
  applyAppTheme({ theme, mode });
  return values;
}

describe('application theme contrast', () => {
  afterEach(() => vi.unstubAllGlobals());

  it.each(
    APP_THEME_PRESETS.flatMap(preset =>
      (['light', 'dark'] as const).map(mode => [preset.id, mode] as const)
    )
  )('%s %s keeps text tokens readable', (theme, mode) => {
    const colors = applyTheme(theme, mode);
    const muted = colors.get('--muted-foreground')!;
    const surfaces = [
      colors.get('--background')!,
      colors.get('--surface')!,
      colors.get('--content-background')!,
      colors.get('--sidebar-background')!,
    ];

    surfaces.forEach(surface =>
      expect(contrast(muted, surface)).toBeGreaterThanOrEqual(4.5)
    );
    expect(
      contrast(
        colors.get('--field-placeholder')!,
        colors.get('--field-background')!
      )
    ).toBeGreaterThanOrEqual(4.5);
    expect(
      contrast(colors.get('--accent')!, colors.get('--accent-foreground')!)
    ).toBeGreaterThanOrEqual(4.5);
    expect(colors.get('--primary')).toBe(colors.get('--accent'));
    expect(
      contrast(
        colors.get('--accent-hover')!,
        colors.get('--accent-foreground')!
      )
    ).toBeGreaterThanOrEqual(4.5);
  });
});
