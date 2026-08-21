import type { TextFontFamily, TextFontSize } from '@/types/editor';

export const FONT_SIZES: number[] = [
  12, 16, 20, 28, 32, 40, 48, 64, 72, 84, 92,
];

export const FONT_FAMILIES: {
  value: TextFontFamily;
  label: string;
  fontFamily: string;
}[] = [
  { value: 'serif', label: 'Serif', fontFamily: 'Georgia, serif' },
  { value: 'mono', label: 'Mono', fontFamily: 'Menlo, monospace' },
  {
    value: 'comic',
    label: 'Comic',
    fontFamily: '"Comic Sans MS", cursive, sans-serif',
  },
];

export const getFontFamilyCSS = (family: TextFontFamily): string => {
  return (
    FONT_FAMILIES.find(f => f.value === family)?.fontFamily ||
    'Arial, sans-serif'
  );
};

export const getFontSizePx = (size: TextFontSize): number => {
  return size;
};

export const TEXT_FONT_WEIGHT = 500;
export const TEXT_BG_COLOR = 'rgba(0, 0, 0, 0.75)';
export const TEXT_BG_PADDING_X = 8;
export const TEXT_BG_PADDING_Y = 4;
export const TEXT_BG_BORDER_RADIUS = 4;
export const SELECTION_BORDER_COLOR = 'rgba(0, 122, 255, 0.8)';
export const SELECTION_BORDER_WIDTH = 2;

let measureCanvas: HTMLCanvasElement | null = null;
let measureContext: CanvasRenderingContext2D | null = null;

export const measureText = (
  text: string,
  fontSize: number,
  fontFamily: string,
  fontWeight: number = TEXT_FONT_WEIGHT
): { width: number; height: number } => {
  if (!measureCanvas) {
    measureCanvas = document.createElement('canvas');
    measureContext = measureCanvas.getContext('2d');
  }

  if (!measureContext) {
    return {
      width: text.length * fontSize * 0.6,
      height: fontSize,
    };
  }

  measureContext.font = `${fontWeight} ${fontSize}px ${fontFamily}`;
  const metrics = measureContext.measureText(text || 'M');

  const height =
    metrics.fontBoundingBoxAscent !== undefined &&
    metrics.fontBoundingBoxDescent !== undefined
      ? metrics.fontBoundingBoxAscent + metrics.fontBoundingBoxDescent
      : fontSize;

  return {
    width: text ? metrics.width : 0,
    height,
  };
};
