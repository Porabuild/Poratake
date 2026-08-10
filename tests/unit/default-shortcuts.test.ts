import { describe, it, expect, afterEach, afterAll, vi } from 'vitest';

const originalPlatform = process.platform;

function setPlatform(platform: string | undefined): void {
  Object.defineProperty(process, 'platform', {
    value: platform,
    configurable: true,
  });
}

function setRendererPlatform(platform: string): void {
  Object.defineProperty(globalThis, 'window', {
    value: { appPlatform: platform },
    configurable: true,
  });
}

async function loadDefaults(platform: string | undefined) {
  vi.resetModules();
  setPlatform(platform);
  return import('@/types/settings');
}

describe('default global shortcuts', () => {
  afterEach(() => {
    Reflect.deleteProperty(globalThis, 'window');
  });

  afterAll(() => {
    setPlatform(originalPlatform);
  });

  it('uses Alt+Shift on Windows', async () => {
    const { DEFAULT_SETTINGS, DEFAULT_GLOBAL_SHORTCUT_MODIFIERS } =
      await loadDefaults('win32');

    expect(DEFAULT_GLOBAL_SHORTCUT_MODIFIERS).toBe('Alt+Shift');
    expect(DEFAULT_SETTINGS.shortcuts.screenshot).toEqual({
      area: 'Alt+Shift+4',
      window: 'Alt+Shift+5',
      screen: 'Alt+Shift+3',
    });
    expect(DEFAULT_SETTINGS.shortcuts.allInOne).toBe('Alt+Shift+S');
  });

  it('uses Cmd+Shift on macOS', async () => {
    const { DEFAULT_SETTINGS, DEFAULT_GLOBAL_SHORTCUT_MODIFIERS } =
      await loadDefaults('darwin');

    expect(DEFAULT_GLOBAL_SHORTCUT_MODIFIERS).toBe('CommandOrControl+Shift');
    expect(DEFAULT_SETTINGS.shortcuts.screenshot).toEqual({
      area: 'CommandOrControl+Shift+4',
      window: 'CommandOrControl+Shift+5',
      screen: 'CommandOrControl+Shift+3',
    });
    expect(DEFAULT_SETTINGS.shortcuts.allInOne).toBe('Alt+Shift+S');
  });

  it('falls back to the renderer platform when process is unavailable', async () => {
    setRendererPlatform('win32');
    const { DEFAULT_GLOBAL_SHORTCUT_MODIFIERS } = await loadDefaults(undefined);

    expect(DEFAULT_GLOBAL_SHORTCUT_MODIFIERS).toBe('Alt+Shift');
  });

  it('keeps editor action shortcuts on the primary modifier', async () => {
    const { DEFAULT_SETTINGS, DEFAULT_UPLOAD_TO_CLOUD_SHORTCUT } =
      await loadDefaults('win32');

    expect(DEFAULT_UPLOAD_TO_CLOUD_SHORTCUT).toBe('CommandOrControl+Shift+U');
    expect(DEFAULT_SETTINGS.shortcuts.editorActions.uploadToCloud).toBe(
      'CommandOrControl+Shift+U'
    );
  });
});
