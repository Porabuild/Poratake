import { describe, expect, it } from 'vitest';

import {
  getWindowFrameCornerRadius,
  isWindowsFrame,
  WINDOW_FRAME_THEMES,
} from '@/renderer/utils/window-frame';

describe('window frame styles', () => {
  it('provides light and dark themes for macOS and Windows', () => {
    expect(Object.keys(WINDOW_FRAME_THEMES)).toEqual([
      'macos-light',
      'macos-dark',
      'windows-light',
      'windows-dark',
    ]);
  });

  it('identifies Windows frames', () => {
    expect(isWindowsFrame('windows-light')).toBe(true);
    expect(isWindowsFrame('windows-dark')).toBe(true);
    expect(isWindowsFrame('macos-light')).toBe(false);
    expect(isWindowsFrame('none')).toBe(false);
  });

  it('uses platform-specific corner radii', () => {
    expect(getWindowFrameCornerRadius('windows-light')).toBe(8);
    expect(getWindowFrameCornerRadius('macos-light')).toBe(10);
  });
});
