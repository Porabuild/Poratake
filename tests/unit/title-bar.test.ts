import { describe, it, expect, vi, beforeEach, afterAll } from 'vitest';
import { EventEmitter } from 'events';

const nativeTheme = Object.assign(new EventEmitter(), {
  shouldUseDarkColors: true,
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
        color: '#1f1f1f',
        symbolColor: '#f8f8f8',
        height: TITLE_BAR_HEIGHT,
      },
    });
    expect(
      titleBarWindowOptions({ height: 32, surface: 'background' })
    ).toEqual({
      titleBarStyle: 'hidden',
      titleBarOverlay: {
        color: '#181818',
        symbolColor: '#f8f8f8',
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

  it('resolves the surface colors from the active theme', async () => {
    const { titleBarColors } = await loadTitleBar('win32');

    expect(titleBarColors('card')).toEqual({
      color: '#1f1f1f',
      symbolColor: '#f8f8f8',
    });

    nativeTheme.shouldUseDarkColors = false;
    expect(titleBarColors('card')).toEqual({
      color: '#ffffff',
      symbolColor: '#000000',
    });
    expect(titleBarColors('background')).toEqual({
      color: '#ffffff',
      symbolColor: '#000000',
    });
  });

  it('reapplies the Windows overlay on show and on theme changes', async () => {
    const { trackTitleBarTheme } = await loadTitleBar('win32');
    const window = fakeWindow();

    trackTitleBarTheme(window as never, { surface: 'background', height: 32 });
    expect(window.setTitleBarOverlay).not.toHaveBeenCalled();

    window.emit('ready-to-show');
    expect(window.setTitleBarOverlay).toHaveBeenCalledWith({
      color: '#181818',
      symbolColor: '#f8f8f8',
      height: 32,
    });
    expect(window.setBackgroundColor).not.toHaveBeenCalled();

    nativeTheme.shouldUseDarkColors = false;
    nativeTheme.emit('updated');
    expect(window.setTitleBarOverlay).toHaveBeenLastCalledWith({
      color: '#ffffff',
      symbolColor: '#000000',
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

    expect(window.setBackgroundColor).toHaveBeenCalledWith('#181818');
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
