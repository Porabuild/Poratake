import { describe, it, expect } from 'vitest';
import { isFeatureSupportedOn } from '@/types/capabilities';

describe('isFeatureSupportedOn', () => {
  it('supports every feature on macOS', () => {
    expect(isFeatureSupportedOn('darwin', 'recording')).toBe(true);
    expect(isFeatureSupportedOn('darwin', 'ocr')).toBe(true);
    expect(isFeatureSupportedOn('darwin', 'screenshot-area')).toBe(true);
    expect(isFeatureSupportedOn('darwin', 'screenshot-window')).toBe(true);
  });

  it('defaults to supported when platform is unknown', () => {
    expect(isFeatureSupportedOn(undefined, 'recording')).toBe(true);
    expect(isFeatureSupportedOn(undefined, 'screenshot-area')).toBe(true);
  });

  it('supports core screenshot features on Windows', () => {
    expect(isFeatureSupportedOn('win32', 'screenshot-screen')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'screenshot-area')).toBe(true);
  });

  it('supports daemon-backed native features on Windows', () => {
    expect(isFeatureSupportedOn('win32', 'ocr')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'qrcode')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'desktop-icons')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'desktop-wallpaper')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'screenshot-window')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'timer-capture')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'display-selector')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'freeze-screen')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'scroll-capture')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'all-in-one')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'print')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'recording')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'video-editor')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'transcription')).toBe(true);
  });

  it('gates unported features off on Windows', () => {
    expect(isFeatureSupportedOn('win32', 'capture-sound')).toBe(false);
  });

  it('gates daemon-backed features off on Linux', () => {
    expect(isFeatureSupportedOn('linux', 'recording')).toBe(false);
    expect(isFeatureSupportedOn('linux', 'ocr')).toBe(false);
    expect(isFeatureSupportedOn('linux', 'desktop-icons')).toBe(false);
    expect(isFeatureSupportedOn('linux', 'desktop-wallpaper')).toBe(false);
    expect(isFeatureSupportedOn('linux', 'screenshot-area')).toBe(true);
  });
});
