export const TAILWIND_COLORS = {
  ROSE: '#f43f5e',
  ORANGE: '#f97316',
  AMBER: '#f59e0b',
  GREEN: '#22c55e',
  EMERALD: '#10b981',
  SKY: '#3b82f6',
  VIOLET: '#8b5cf6',
  PURPLE: '#a855f7',
  INDIGO: '#6366f1',
  SLATE: '#64748b',
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
