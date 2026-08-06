import { useMemo } from 'react';
import type { HighlightColor, RedactStyle, ToolType } from '@/types/editor';

interface BrushCursorOptions {
  activeTool: ToolType;
  redactStyle?: RedactStyle;
  highlightColor?: HighlightColor;
}

export function useBrushCursor({
  activeTool,
  redactStyle = 'pixelate',
  highlightColor = '#FFFF00',
}: BrushCursorOptions): string {
  return useMemo(() => {
    const crosshairTools = ['pen', 'rectangle', 'circle', 'line', 'arrow'];
    const specialCursorTools = ['redact', 'highlight'];

    if (crosshairTools.includes(activeTool)) {
      return 'crosshair';
    }

    if (!specialCursorTools.includes(activeTool)) {
      return getCursorForTool(activeTool);
    }

    if (activeTool === 'redact') {
      const svg = generateRedactCursorSvg(redactStyle);
      const encodedSvg = encodeURIComponent(svg);
      return `url("data:image/svg+xml,${encodedSvg}") 10 10, crosshair`;
    }

    if (activeTool === 'highlight') {
      const svg = generateHighlightCursorSvg(highlightColor);
      const encodedSvg = encodeURIComponent(svg);
      return `url("data:image/svg+xml,${encodedSvg}") 10 10, crosshair`;
    }

    return 'crosshair';
  }, [activeTool, redactStyle, highlightColor]);
}

function getCursorForTool(tool: ToolType): string {
  switch (tool) {
    case 'select':
      return 'default';
    case 'text':
      return 'text';
    case 'crop':
      return 'crosshair';
    case 'number':
      return 'crosshair';
    case 'redact':
      return 'crosshair';
    case 'wallpaper':
      return 'default';
    default:
      return 'default';
  }
}

function generateHighlightCursorSvg(color: string): string {
  const width = 20;
  const height = 24;
  const brushWidth = 6;
  const brushHeight = 16;
  const centerX = width / 2;
  const centerY = height / 2;
  const outlineColor = getContrastingOutline(color);

  return `
    <svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
      <rect 
        x="${centerX - brushWidth / 2}" 
        y="${centerY - brushHeight / 2}" 
        width="${brushWidth}" 
        height="${brushHeight}" 
        fill="${color}" 
        fill-opacity="0.6"
        stroke="${outlineColor}" 
        stroke-width="1"
        rx="1"
      />
      <line x1="${centerX - 4}" y1="${centerY}" x2="${centerX + 4}" y2="${centerY}" stroke="${outlineColor}" stroke-width="0.5"/>
    </svg>
  `.trim();
}

function generateRedactCursorSvg(style: RedactStyle): string {
  const size = 20;
  const center = size / 2;

  switch (style) {
    case 'pixelate':
      return `
        <svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
          <rect x="2" y="2" width="6" height="6" fill="rgba(100,100,100,0.8)"/>
          <rect x="8" y="2" width="6" height="6" fill="rgba(150,150,150,0.8)"/>
          <rect x="2" y="8" width="6" height="6" fill="rgba(150,150,150,0.8)"/>
          <rect x="8" y="8" width="6" height="6" fill="rgba(100,100,100,0.8)"/>
          <rect x="14" y="2" width="4" height="4" fill="rgba(120,120,120,0.6)"/>
          <rect x="2" y="14" width="4" height="4" fill="rgba(120,120,120,0.6)"/>
          <circle cx="${center}" cy="${center}" r="1" fill="white"/>
        </svg>
      `.trim();

    case 'blur':
      return `
        <svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
          <defs>
            <radialGradient id="blurGrad">
              <stop offset="0%" stop-color="rgba(128,128,128,0.9)"/>
              <stop offset="70%" stop-color="rgba(128,128,128,0.3)"/>
              <stop offset="100%" stop-color="rgba(128,128,128,0)"/>
            </radialGradient>
          </defs>
          <circle cx="${center}" cy="${center}" r="8" fill="url(#blurGrad)"/>
          <circle cx="${center}" cy="${center}" r="1" fill="white"/>
        </svg>
      `.trim();

    case 'blackout':
      return `
        <svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
          <rect x="3" y="5" width="14" height="10" fill="black" rx="1"/>
          <circle cx="${center}" cy="${center}" r="1" fill="white"/>
        </svg>
      `.trim();

    default:
      return `
        <svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
          <rect x="4" y="4" width="12" height="12" fill="rgba(100,100,100,0.5)" stroke="white" stroke-width="1"/>
        </svg>
      `.trim();
  }
}

function getContrastingOutline(color: string): string {
  const rgb = parseColor(color);
  if (!rgb) return 'rgba(0,0,0,0.5)';

  const luminance = (0.299 * rgb.r + 0.587 * rgb.g + 0.114 * rgb.b) / 255;

  return luminance > 0.5 ? 'rgba(0,0,0,0.7)' : 'rgba(255,255,255,0.9)';
}

function parseColor(color: string): { r: number; g: number; b: number } | null {
  if (color.startsWith('#')) {
    const hex = color.slice(1);
    if (hex.length === 3) {
      return {
        r: parseInt(hex[0] + hex[0], 16),
        g: parseInt(hex[1] + hex[1], 16),
        b: parseInt(hex[2] + hex[2], 16),
      };
    }
    if (hex.length === 6) {
      return {
        r: parseInt(hex.slice(0, 2), 16),
        g: parseInt(hex.slice(2, 4), 16),
        b: parseInt(hex.slice(4, 6), 16),
      };
    }
  }

  const rgbMatch = color.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
  if (rgbMatch) {
    return {
      r: parseInt(rgbMatch[1], 10),
      g: parseInt(rgbMatch[2], 10),
      b: parseInt(rgbMatch[3], 10),
    };
  }

  return null;
}
