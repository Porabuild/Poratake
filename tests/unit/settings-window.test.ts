import { describe, it, expect, vi, beforeEach } from 'vitest';

const browserWindows: MockBrowserWindow[] = [];

class MockBrowserWindow {
  windowHandlers: Record<string, ((...a: unknown[]) => unknown)[]> = {};

  webContents = {
    id: 1,
    on: vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
      this.windowHandlers[`wc:${event}`] ??= [];
      this.windowHandlers[`wc:${event}`].push(cb);
    }),
    once: vi.fn(),
    send: vi.fn(),
  };

  destroyedFlag = false;
  options: Electron.BrowserWindowConstructorOptions;
  show = vi.fn();
  focus = vi.fn();
  loadURL = vi.fn();
  loadFile = vi.fn();
  isDestroyed = vi.fn(() => this.destroyedFlag);
  on = vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
    this.windowHandlers[event] ??= [];
    this.windowHandlers[event].push(cb);
  });
  once = vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
    this.windowHandlers[event] ??= [];
    this.windowHandlers[event].push(cb);
  });

  constructor(opts: Electron.BrowserWindowConstructorOptions) {
    this.options = opts;
    browserWindows.push(this);
  }
}

vi.mock('electron', () => ({
  BrowserWindow: MockBrowserWindow,
  app: { focus: vi.fn() },
  screen: {
    getPrimaryDisplay: () => ({
      workAreaSize: { width: 1920, height: 1080 },
    }),
  },
}));

vi.mock('@/main/utils/env', () => ({
  isDev: true,
  devServerUrl: 'http://localhost:5173',
}));

vi.mock('@/main/utils/dock', () => ({
  registerDockWindow: vi.fn().mockResolvedValue(undefined),
}));

describe('settings window', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    browserWindows.splice(0);
  });

  it('creates a new settings window when none exists', async () => {
    const { createOrShowSettingsWindow } =
      await import('@/main/settings/window');
    createOrShowSettingsWindow();
    expect(browserWindows.length).toBe(1);
  });

  it('starts supported Windows settings windows on acrylic', async () => {
    const { createOrShowSettingsWindow } =
      await import('@/main/settings/window');
    const { supportsWindowsAcrylic } = await import('@/main/utils/title-bar');
    createOrShowSettingsWindow();

    if (!supportsWindowsAcrylic()) return;

    expect(browserWindows[0].options).toMatchObject({
      backgroundColor: '#00000000',
      backgroundMaterial: 'acrylic',
      titleBarOverlay: { color: '#00000000' },
    });
  });

  it('starts macOS settings windows with sidebar vibrancy', async () => {
    const { createOrShowSettingsWindow } =
      await import('@/main/settings/window');
    createOrShowSettingsWindow();

    if (process.platform !== 'darwin') return;

    expect(browserWindows[0].options).toMatchObject({
      backgroundColor: '#00000000',
      vibrancy: 'sidebar',
      visualEffectState: 'active',
      transparent: true,
    });
  });

  it('reuses existing window on subsequent calls', async () => {
    const { createOrShowSettingsWindow } =
      await import('@/main/settings/window');
    createOrShowSettingsWindow();
    createOrShowSettingsWindow();
    expect(browserWindows.length).toBe(1);
    expect(browserWindows[0].show).toHaveBeenCalled();
    expect(browserWindows[0].focus).toHaveBeenCalled();
  });

  it('navigates to tab on existing window', async () => {
    const { createOrShowSettingsWindow } =
      await import('@/main/settings/window');
    createOrShowSettingsWindow();
    createOrShowSettingsWindow('shortcuts');
    expect(browserWindows[0].webContents.send).toHaveBeenCalledWith(
      'navigate-tab',
      'shortcuts'
    );
  });

  it('loads URL with hash when tab is provided', async () => {
    const { createOrShowSettingsWindow } =
      await import('@/main/settings/window');
    createOrShowSettingsWindow('storage');
    expect(browserWindows[0].loadURL).toHaveBeenCalledWith(
      'http://localhost:5173#storage'
    );
  });

  describe('window event handlers', () => {
    it('did-finish-load sends load event', async () => {
      const { createOrShowSettingsWindow } =
        await import('@/main/settings/window');
      createOrShowSettingsWindow();
      const win = browserWindows[0];
      (win.windowHandlers['wc:did-finish-load'] || []).forEach(cb => cb());
      expect(win.webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({
          type: 'settings',
          params: expect.objectContaining({
            nativeMaterial: expect.any(Boolean),
          }),
        })
      );
    });

    it('ready-to-show registers dock and shows window', async () => {
      const { createOrShowSettingsWindow } =
        await import('@/main/settings/window');
      createOrShowSettingsWindow();
      const win = browserWindows[0];
      const handler = (win.windowHandlers['ready-to-show'] || [])[0];
      await handler();
      expect(win.show).toHaveBeenCalled();
      expect(win.focus).toHaveBeenCalled();
    });

    it('closed handler clears reference', async () => {
      const { createOrShowSettingsWindow } =
        await import('@/main/settings/window');
      createOrShowSettingsWindow();
      const win = browserWindows[0];
      (win.windowHandlers['closed'] || []).forEach(cb => cb());
      // Calling again should make a new window
      createOrShowSettingsWindow();
      expect(browserWindows.length).toBe(2);
    });
  });
});
