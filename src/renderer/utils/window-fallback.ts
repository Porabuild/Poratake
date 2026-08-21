import type { WindowType } from '@/types/window-load';

const TRANSPARENT_WINDOW_TYPES = new Set<WindowType>([
  'area-overlay',
  'recording-control',
  'scroll-capture-overlay',
  'scroll-capture-control',
]);

export function usesTransparentWindowFallback(
  windowType: string | null
): boolean {
  return (
    windowType !== null &&
    TRANSPARENT_WINDOW_TYPES.has(windowType as WindowType)
  );
}
