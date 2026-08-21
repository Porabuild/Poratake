import { screen } from 'electron';
import type { Display, Rectangle } from 'electron';
import type { AreaOverlayResult } from '@/types/area-overlay';
import { clamp } from '@/types/geometry';
import type { Rect } from '@/types/geometry';

export function clipToDisplay(display: Display, rect: Rectangle): Rect | null {
  const x = Math.max(rect.x, display.bounds.x);
  const y = Math.max(rect.y, display.bounds.y);
  const right = Math.min(
    rect.x + rect.width,
    display.bounds.x + display.bounds.width
  );
  const bottom = Math.min(
    rect.y + rect.height,
    display.bounds.y + display.bounds.height
  );

  if (right <= x || bottom <= y) return null;

  return {
    x: Math.round(x - display.bounds.x),
    y: Math.round(y - display.bounds.y),
    width: Math.round(right - x),
    height: Math.round(bottom - y),
  };
}

export function fitToDisplay(
  display: Display,
  rect: Rectangle
): AreaOverlayResult | null {
  const { width: displayWidth, height: displayHeight } = display.bounds;
  const width = clamp(Math.round(rect.width), 1, displayWidth);
  const height = clamp(Math.round(rect.height), 1, displayHeight);

  if (!Number.isFinite(rect.x) || !Number.isFinite(rect.y)) return null;

  return {
    displayId: display.id,
    width,
    height,
    x: clamp(Math.round(rect.x - display.bounds.x), 0, displayWidth - width),
    y: clamp(Math.round(rect.y - display.bounds.y), 0, displayHeight - height),
  };
}

export function presetSelection(
  displays: Display[],
  preset?: Rectangle
): AreaOverlayResult | null {
  if (!preset) return null;

  const display = screen.getDisplayMatching(preset);
  if (!displays.some(item => item.id === display.id)) return null;

  return fitToDisplay(display, preset);
}
