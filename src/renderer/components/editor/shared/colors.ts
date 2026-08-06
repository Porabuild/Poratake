export const TAILWIND_COLORS = {
  ROSE: 'oklch(64.5% 0.246 16.439)',
  ORANGE: 'oklch(70.5% 0.213 47.604)',
  AMBER: 'oklch(76.9% 0.188 70.08)',
  GREEN: 'oklch(72.3% 0.219 149.579)',
  EMERALD: 'oklch(69.6% 0.17 162.48)',
  SKY: 'oklch(62.3% 0.214 259.815)',
  VIOLET: 'oklch(60.6% 0.25 292.717)',
  PURPLE: 'oklch(62.7% 0.265 303.9)',
  INDIGO: 'oklch(58.5% 0.233 277.117)',
  SLATE: 'oklch(55.4% 0.046 257.417)',
  BLACK: '#000000',
  WHITE: '#ffffff',
} as const;

export const COLOR_PALETTE = [
  { name: 'Rose', value: TAILWIND_COLORS.ROSE },
  { name: 'Orange', value: TAILWIND_COLORS.ORANGE },
  { name: 'Amber', value: TAILWIND_COLORS.AMBER },
  { name: 'Green', value: TAILWIND_COLORS.GREEN },
  { name: 'Emerald', value: TAILWIND_COLORS.EMERALD },
  { name: 'Sky', value: TAILWIND_COLORS.SKY },
  { name: 'Violet', value: TAILWIND_COLORS.VIOLET },
  { name: 'Purple', value: TAILWIND_COLORS.PURPLE },
  { name: 'Indigo', value: TAILWIND_COLORS.INDIGO },
  { name: 'Slate', value: TAILWIND_COLORS.SLATE },
  { name: 'Black', value: TAILWIND_COLORS.BLACK },
  { name: 'White', value: TAILWIND_COLORS.WHITE },
] as const;

export const getContrastColor = (hexColor: string): string => {
  const hex = hexColor.replace('#', '');
  const r = parseInt(hex.substring(0, 2), 16);
  const g = parseInt(hex.substring(2, 4), 16);
  const b = parseInt(hex.substring(4, 6), 16);
  const luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return luminance > 0.5 ? '#000000' : '#ffffff';
};

export const SELECTION_STROKE = 'rgba(0, 122, 255, 0.8)';
export const SELECTION_STROKE_WIDTH = 6;
export const SELECTION_BORDER_COLOR = 'rgba(0, 122, 255, 0.8)';
export const SELECTION_BORDER_WIDTH = 2;
