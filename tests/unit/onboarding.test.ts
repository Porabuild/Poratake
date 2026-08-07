import { describe, it, expect, vi, beforeEach } from 'vitest';

type IpcHandler = (...args: unknown[]) => unknown;

const ipcHandlers: Record<string, IpcHandler> = {};
const mockIpcOn = vi.fn((event: string, cb: IpcHandler) => {
  ipcHandlers[event] = cb;
});

const browserWindowInstances: MockBrowserWindow[] = [];

class MockBrowserWindow {
  webContentsHandlers: Record<string, ((...a: unknown[]) => unknown)[]> = {};
  windowHandlers: Record<string, ((...a: unknown[]) => unknown)[]> = {};

  webContents = {
    on: vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
      this.webContentsHandlers[event] ??= [];
      this.webContentsHandlers[event].push(cb);
    }),
    send: vi.fn(),
  };

  focus = vi.fn();
  loadURL = vi.fn();
  loadFile = vi.fn();
  show = vi.fn();
  close = vi.fn(() => {
    (this.windowHandlers['closed'] || []).forEach(cb => cb());
  });
  destroyed = false;
  isDestroyed = vi.fn(() => this.destroyed);

  on = vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
    this.windowHandlers[event] ??= [];
    this.windowHandlers[event].push(cb);
  });

  once = vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
    this.windowHandlers[event] ??= [];
    this.windowHandlers[event].push(cb);
  });

  constructor(_opts: unknown) {
    void _opts;
    browserWindowInstances.push(this);
  }
}

const mockShellOpenExternal = vi.fn();
const mockShellShowItemInFolder = vi.fn();

vi.mock('electron', () => {
  return {
    BrowserWindow: MockBrowserWindow,
    ipcMain: { on: mockIpcOn },
    shell: {
      openExternal: (...a: unknown[]) => mockShellOpenExternal(...a),
      showItemInFolder: (...a: unknown[]) => mockShellShowItemInFolder(...a),
    },
    screen: {
      getPrimaryDisplay: vi.fn(() => ({
        workAreaSize: { width: 1920, height: 1080 },
      })),
    },
    nativeTheme: { shouldUseDarkColors: false },
  };
});

vi.mock('@/main/utils/env', () => ({
  isDev: true,
  devServerUrl: 'http://localhost:5173',
}));

const mockMarkCompleted = vi.fn();
const mockMarkSkipped = vi.fn();
const mockNeedsOnboarding = vi.fn();
vi.mock('@/main/settings', () => ({
  markOnboardingCompleted: () => mockMarkCompleted(),
  markOnboardingSkipped: () => mockMarkSkipped(),
  needsOnboarding: () => mockNeedsOnboarding(),
}));

describe('onboarding', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandlers).forEach(k => delete ipcHandlers[k]);
    browserWindowInstances.splice(0);
  });

  it('creates onboarding window', async () => {
    const { createOnboardingWindow } = await import('@/main/onboarding');
    const win = createOnboardingWindow();
    expect(win).toBeDefined();
    expect(browserWindowInstances.length).toBe(1);
  });

  it('returns existing window on second call', async () => {
    const m = await import('@/main/onboarding');
    const first = m.createOnboardingWindow();
    const second = m.createOnboardingWindow();
    expect(second).toBe(first);
    expect(first.focus).toHaveBeenCalled();
  });

  it('getOnboardingWindow returns current window', async () => {
    const m = await import('@/main/onboarding');
    expect(m.getOnboardingWindow()).toBeNull();
    m.createOnboardingWindow();
    expect(m.getOnboardingWindow()).not.toBeNull();
  });

  it('closeOnboardingWindow closes and clears the reference', async () => {
    const m = await import('@/main/onboarding');
    m.createOnboardingWindow();
    m.closeOnboardingWindow();
    expect(m.getOnboardingWindow()).toBeNull();
  });

  it('init wires up ipc handlers', async () => {
    const { init } = await import('@/main/onboarding');
    init();
    expect(ipcHandlers['onboarding:complete']).toBeDefined();
    expect(ipcHandlers['onboarding:skip']).toBeDefined();
    expect(ipcHandlers['shell:open-external']).toBeDefined();
    expect(ipcHandlers['shell:reveal-in-finder']).toBeDefined();
  });

  it('onboarding:complete marks completed and runs callback', async () => {
    const m = await import('@/main/onboarding');
    m.createOnboardingWindow();
    const cb = vi.fn(async () => {});
    m.setOnCompletedCallback(cb);
    m.init();
    await ipcHandlers['onboarding:complete']();
    expect(mockMarkCompleted).toHaveBeenCalled();
    expect(cb).toHaveBeenCalled();
  });

  it('onboarding:skip marks skipped and runs callback', async () => {
    const m = await import('@/main/onboarding');
    m.createOnboardingWindow();
    const cb = vi.fn(async () => {});
    m.setOnCompletedCallback(cb);
    m.init();
    await ipcHandlers['onboarding:skip']();
    expect(mockMarkSkipped).toHaveBeenCalled();
    expect(cb).toHaveBeenCalled();
  });

  it('shell:open-external opens URL', async () => {
    const m = await import('@/main/onboarding');
    m.init();
    ipcHandlers['shell:open-external']({}, 'https://capty.app');
    expect(mockShellOpenExternal).toHaveBeenCalledWith('https://capty.app');
  });

  it('shell:reveal-in-finder shows item in folder', async () => {
    const m = await import('@/main/onboarding');
    m.init();
    ipcHandlers['shell:reveal-in-finder']({}, '/path/to/My.capty');
    expect(mockShellShowItemInFolder).toHaveBeenCalledWith(
      '/path/to/My.capty/recording.mov'
    );
  });

  describe('showOnboardingOrRun', () => {
    it('shows onboarding when needed', async () => {
      mockNeedsOnboarding.mockReturnValue(true);
      const m = await import('@/main/onboarding');
      const cb = vi.fn(async () => {});
      await m.showOnboardingOrRun(cb);
      expect(browserWindowInstances.length).toBe(1);
      expect(cb).not.toHaveBeenCalled();
    });

    it('runs callback immediately when onboarding not needed', async () => {
      mockNeedsOnboarding.mockReturnValue(false);
      const m = await import('@/main/onboarding');
      const cb = vi.fn(async () => {});
      await m.showOnboardingOrRun(cb);
      expect(browserWindowInstances.length).toBe(0);
      expect(cb).toHaveBeenCalled();
    });
  });

  describe('window lifecycle', () => {
    it('did-finish-load sends load event', async () => {
      const m = await import('@/main/onboarding');
      m.createOnboardingWindow();
      const win = browserWindowInstances[0];
      const handlers = win.webContentsHandlers['did-finish-load'] || [];
      handlers.forEach(cb => cb());
      expect(win.webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({ type: 'onboarding' })
      );
    });

    it('ready-to-show shows window', async () => {
      const m = await import('@/main/onboarding');
      m.createOnboardingWindow();
      const win = browserWindowInstances[0];
      const handlers = win.windowHandlers['ready-to-show'] || [];
      handlers.forEach(cb => cb());
      expect(win.show).toHaveBeenCalled();
    });

    it('closed handler clears window reference', async () => {
      const m = await import('@/main/onboarding');
      m.createOnboardingWindow();
      const win = browserWindowInstances[0];
      const handlers = win.windowHandlers['closed'] || [];
      handlers.forEach(cb => cb());
      expect(m.getOnboardingWindow()).toBeNull();
    });
  });
});
