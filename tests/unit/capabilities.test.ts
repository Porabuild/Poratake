import { describe, it, expect } from 'vitest';
import { isFeatureSupportedOn } from '@/types/capabilities';
import { PLATFORM_CAPABILITIES } from '@/types/capabilities.generated';

describe('isFeatureSupportedOn', () => {
  it('supports every feature on macOS', () => {
    expect(isFeatureSupportedOn('darwin', 'recording')).toBe(true);
    expect(isFeatureSupportedOn('darwin', 'ocr')).toBe(true);
    expect(isFeatureSupportedOn('darwin', 'screenshot-area')).toBe(true);
    expect(isFeatureSupportedOn('darwin', 'screenshot-window')).toBe(true);
  });

  it('does not infer capabilities when platform is unknown', () => {
    expect(isFeatureSupportedOn(undefined, 'recording')).toBe(false);
    expect(isFeatureSupportedOn(undefined, 'screenshot-area')).toBe(false);
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

  it('keeps Electron Linux disabled until it uses daemon-linux', () => {
    expect(isFeatureSupportedOn('linux', 'screenshot-area')).toBe(false);
    expect(isFeatureSupportedOn('linux', 'qrcode')).toBe(false);
  });

  it('defines explicit GPUI Linux session capabilities', () => {
    const x11: readonly string[] = PLATFORM_CAPABILITIES.linuxX11;
    const wayland: readonly string[] = PLATFORM_CAPABILITIES.linuxWayland;
    expect(x11.includes('screenshot-window')).toBe(true);
    expect(x11.includes('scroll-capture')).toBe(true);
    expect(x11.includes('display-selector')).toBe(true);
    expect(x11.includes('recording')).toBe(true);
    expect(wayland.includes('screenshot-window')).toBe(false);
    expect(wayland.includes('screenshot-screen')).toBe(true);
    expect(wayland.includes('screenshot-area')).toBe(true);
    expect(wayland.includes('display-selector')).toBe(false);
    expect(wayland.includes('recording')).toBe(true);
    expect(x11.includes('video-editor')).toBe(true);
    expect(wayland.includes('video-editor')).toBe(true);
  });

  it('keeps color picker support aligned with GPUI', () => {
    expect(isFeatureSupportedOn('darwin', 'color-picker')).toBe(true);
    expect(isFeatureSupportedOn('win32', 'color-picker')).toBe(true);
    const x11: readonly string[] = PLATFORM_CAPABILITIES.linuxX11;
    const wayland: readonly string[] = PLATFORM_CAPABILITIES.linuxWayland;
    expect(x11.includes('color-picker')).toBe(true);
    expect(wayland.includes('color-picker')).toBe(false);
  });
});
