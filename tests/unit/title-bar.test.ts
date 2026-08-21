import { describe, it, expect, vi, beforeEach, afterAll } from 'vitest';
import { EventEmitter } from 'events';

const nativeTheme = Object.assign(new EventEmitter(), {
  shouldUseDarkColors: true,
  themeSource: 'system',
});

vi.mock('electron', () => ({
  nativeTheme,
}));

const originalPlatform = process.platform;

function setPlatform(platform: string): void {
  Object.defineProperty(process, 'platform', {
    value: platform,
    configurable: true,
  });
}

async function loadTitleBar(platform: string) {
  vi.resetModules();
  setPlatform(platform);
  return import('@/main/utils/title-bar');
}

function fakeWindow() {
  const handlers = new Map<string, () => void>();
  return {
    isDestroyed: vi.fn(() => false),
    setTitleBarOverlay: vi.fn(),
    setBackgroundColor: vi.fn(),
    once: vi.fn((event: string, handler: () => void) => {
      handlers.set(event, handler);
    }),
    emit: (event: string) => handlers.get(event)?.(),
  };
}

describe('title-bar', () => {
  beforeEach(() => {
    nativeTheme.shouldUseDarkColors = true;
    nativeTheme.removeAllListeners();
  });

  afterAll(() => {
    setPlatform(originalPlatform);
  });

  it('keeps the inset traffic lights on macOS and adds no overlay', async () => {
    const { titleBarWindowOptions } = await loadTitleBar('darwin');

    expect(titleBarWindowOptions()).toEqual({ titleBarStyle: 'hiddenInset' });
    expect(
      titleBarWindowOptions({ trafficLightPosition: { x: 16, y: 18 } })
    ).toEqual({
      titleBarStyle: 'hiddenInset',
      trafficLightPosition: { x: 16, y: 18 },
    });
  });

  it('hides the Windows header and sizes the overlay to the toolbar', async () => {
    const { titleBarWindowOptions, TITLE_BAR_HEIGHT } =
      await loadTitleBar('win32');

    expect(titleBarWindowOptions()).toEqual({
      titleBarStyle: 'hidden',
      titleBarOverlay: {
        color: '#0e0e14',
        symbolColor: '#fafafa',
        height: TITLE_BAR_HEIGHT,
      },
    });
    expect(
      titleBarWindowOptions({ height: 32, surface: 'background' })
    ).toEqual({
      titleBarStyle: 'hidden',
      titleBarOverlay: {
        color: '#070709',
        symbolColor: '#fafafa',
        height: 32,
      },
    });
  });

  it('drops the macOS traffic light position from the Windows overlay', async () => {
    const { titleBarWindowOptions } = await loadTitleBar('win32');

    expect(
      titleBarWindowOptions({ trafficLightPosition: { x: 16, y: 18 } })
    ).not.toHaveProperty('trafficLightPosition');
  });

  it('uses a transparent Windows overlay over native acrylic', async () => {
    const { titleBarWindowOptions } = await loadTitleBar('win32');

    expect(titleBarWindowOptions({ transparent: true })).toEqual({
      titleBarStyle: 'hidden',
      titleBarOverlay: {
        color: '#00000000',
        symbolColor: '#fafafa',
        height: 40,
      },
    });
  });

  it('requires Windows 11 22H2 for acrylic', async () => {
    const { supportsWindowsAcrylic } = await loadTitleBar('win32');

    expect(supportsWindowsAcrylic('10.0.22000')).toBe(false);
    expect(supportsWindowsAcrylic('10.0.22621')).toBe(true);
  });

  it('creates macOS sidebar vibrancy options', async () => {
    const { nativeWindowMaterialOptions, supportsNativeWindowMaterial } =
      await loadTitleBar('darwin');

    expect(supportsNativeWindowMaterial()).toBe(true);
    expect(nativeWindowMaterialOptions()).toEqual({
      vibrancy: 'sidebar',
      visualEffectState: 'active',
      transparent: true,
    });
  });

  it('creates Windows acrylic options only on supported builds', async () => {
    const { nativeWindowMaterialOptions } = await loadTitleBar('win32');

    expect(nativeWindowMaterialOptions('10.0.22000')).toEqual({});
    expect(nativeWindowMaterialOptions('10.0.22621')).toEqual({
      backgroundMaterial: 'acrylic',
    });
  });

  it('resolves the surface colors from the active theme', async () => {
    const { titleBarColors, applyTitleBarAppearance } =
      await loadTitleBar('win32');
    applyTitleBarAppearance({ mode: 'system', theme: 'default' });

    expect(titleBarColors('card')).toEqual({
      color: '#0e0e14',
      symbolColor: '#fafafa',
    });

    nativeTheme.shouldUseDarkColors = false;
    expect(titleBarColors('card')).toEqual({
      color: '#fafafb',
      symbolColor: '#18181b',
    });
    expect(titleBarColors('background')).toEqual({
      color: '#f1f1f4',
      symbolColor: '#18181b',
    });
  });

  it('reapplies the Windows overlay on show and on theme changes', async () => {
    const { trackTitleBarTheme, applyTitleBarAppearance } =
      await loadTitleBar('win32');
    applyTitleBarAppearance({ mode: 'system', theme: 'default' });
    const window = fakeWindow();

    trackTitleBarTheme(window as never, { surface: 'background', height: 32 });
    expect(window.setTitleBarOverlay).not.toHaveBeenCalled();

    window.emit('ready-to-show');
    expect(window.setTitleBarOverlay).toHaveBeenCalledWith({
      color: '#070709',
      symbolColor: '#fafafa',
      height: 32,
    });
    expect(window.setBackgroundColor).not.toHaveBeenCalled();

    nativeTheme.shouldUseDarkColors = false;
    nativeTheme.emit('updated');
    expect(window.setTitleBarOverlay).toHaveBeenLastCalledWith({
      color: '#f1f1f4',
      symbolColor: '#18181b',
      height: 32,
    });
  });

  it('syncs the window background only when asked', async () => {
    const { trackTitleBarTheme } = await loadTitleBar('win32');
    const window = fakeWindow();

    trackTitleBarTheme(window as never, {
      surface: 'background',
      syncBackground: true,
    });
    window.emit('ready-to-show');

    expect(window.setBackgroundColor).toHaveBeenCalledWith('#070709');
  });

  it('applies the selected preset to native title bars', async () => {
    const { applyTitleBarAppearance, titleBarColors } =
      await loadTitleBar('win32');

    applyTitleBarAppearance({ mode: 'dark', theme: 'github' });

    expect(titleBarColors('card')).toEqual({
      color: '#161b22',
      symbolColor: '#e6edf3',
    });
  });

  it('keeps the Windows native theme on system so tray surfaces follow the OS', async () => {
    const { applyTitleBarAppearance } = await loadTitleBar('win32');

    applyTitleBarAppearance({ mode: 'dark', theme: 'default' });

    expect(nativeTheme.themeSource).toBe('system');
  });

  it('follows the app mode for the Windows overlay even when the system is light', async () => {
    const { applyTitleBarAppearance, titleBarColors } =
      await loadTitleBar('win32');

    nativeTheme.shouldUseDarkColors = false;
    applyTitleBarAppearance({ mode: 'dark', theme: 'default' });

    expect(titleBarColors('card')).toEqual({
      color: '#0e0e14',
      symbolColor: '#fafafa',
    });
  });

  it('syncs the macOS native theme with the app mode', async () => {
    const { applyTitleBarAppearance } = await loadTitleBar('darwin');

    applyTitleBarAppearance({ mode: 'dark', theme: 'github' });

    expect(nativeTheme.themeSource).toBe('dark');
  });

  it('stops listening once the window is closed', async () => {
    const { trackTitleBarTheme } = await loadTitleBar('win32');
    const window = fakeWindow();

    trackTitleBarTheme(window as never);
    expect(nativeTheme.listenerCount('updated')).toBe(1);

    window.emit('closed');
    expect(nativeTheme.listenerCount('updated')).toBe(0);
  });

  it('skips overlay tracking on macOS', async () => {
    const { trackTitleBarTheme } = await loadTitleBar('darwin');
    const window = fakeWindow();

    trackTitleBarTheme(window as never);

    expect(window.once).not.toHaveBeenCalled();
    expect(nativeTheme.listenerCount('updated')).toBe(0);
  });
});
