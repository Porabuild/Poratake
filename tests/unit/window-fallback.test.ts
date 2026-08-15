import { describe, expect, it } from 'vitest';
import { usesTransparentWindowFallback } from '@/renderer/utils/window-fallback';

describe('window fallback', () => {
  it.each([
    'area-overlay',
    'recording-control',
    'scroll-capture-overlay',
    'scroll-capture-control',
  ])('keeps the %s window transparent while loading', windowType => {
    expect(usesTransparentWindowFallback(windowType)).toBe(true);
  });

  it.each([null, 'screenshot', 'settings', 'video-editor'])(
    'keeps the normal fallback for %s',
    windowType => {
      expect(usesTransparentWindowFallback(windowType)).toBe(false);
    }
  );
});
