export const FONT_SIZE_OPTIONS = [
  { value: 'small', label: 'Small' },
  { value: 'medium', label: 'Medium' },
  { value: 'large', label: 'Large' },
] as const;

export const FONT_SIZES = {
  small: 108,
  medium: 144,
  large: 180,
} as const;

export const CANVAS_CONSTANTS = {
  paddingVertical: 32,
  paddingHorizontal: 48,
  marginEdge: 40,
  cornerRadius: 40,
} as const;
