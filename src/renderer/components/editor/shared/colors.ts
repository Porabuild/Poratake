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

export const SELECTION_STROKE =
  'color-mix(in srgb, var(--primary) 80%, transparent)';
export const SELECTION_STROKE_WIDTH = 6;
export const SELECTION_BORDER_COLOR = SELECTION_STROKE;
export const SELECTION_BORDER_WIDTH = 2;
