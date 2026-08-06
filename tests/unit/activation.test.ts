import { describe, it, expect, vi, beforeEach } from 'vitest';

type IpcHandler = (...args: unknown[]) => unknown;

const ipcHandlers: Record<string, IpcHandler> = {};
const mockIpcOn = vi.fn((event: string, cb: IpcHandler) => {
  ipcHandlers[event] = cb;
});

const browserWindowInstances: MockBrowserWindow[] = [];

class MockBrowserWindow {
  static getAllWindows = vi.fn(() => browserWindowInstances.slice());

  webContentsHandlers: Record<string, ((...a: unknown[]) => unknown)[]> = {};
  windowHandlers: Record<string, ((...a: unknown[]) => unknown)[]> = {};

  webContents = {
    on: vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
      this.webContentsHandlers[event] ??= [];
      this.webContentsHandlers[event].push(cb);
    }),
    send: vi.fn(),
    setWindowOpenHandler: vi.fn(),
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

const mockAppFocus = vi.fn();
const mockNotificationShow = vi.fn();

vi.mock('electron', () => {
  return {
    BrowserWindow: MockBrowserWindow,
    ipcMain: { on: mockIpcOn },
    shell: { openExternal: vi.fn() },
    app: { focus: mockAppFocus },
    screen: {
      getPrimaryDisplay: vi.fn(() => ({
        workAreaSize: { width: 1920, height: 1080 },
      })),
    },
    Notification: class {
      static isSupported = vi.fn(() => true);
      show = mockNotificationShow;
    },
  };
});

vi.mock('@/main/utils/env', () => ({
  isDev: true,
  devServerUrl: 'http://localhost:5173',
}));

describe('activation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandlers).forEach(k => delete ipcHandlers[k]);
    browserWindowInstances.splice(0);
  });

  it('creates an activation window with expected dimensions', async () => {
    const { createActivationWindow } = await import('@/main/activation');
    const win = createActivationWindow();
    expect(win).toBeDefined();
    expect(browserWindowInstances.length).toBe(1);
    expect(win.loadURL).toHaveBeenCalledWith('http://localhost:5173');
  });

  it('returns existing window on second call', async () => {
    const m = await import('@/main/activation');
    const first = m.createActivationWindow();
    const second = m.createActivationWindow();
    expect(second).toBe(first);
    expect(first.focus).toHaveBeenCalled();
  });

  it('getActivationWindow returns current window', async () => {
    const m = await import('@/main/activation');
    expect(m.getActivationWindow()).toBeNull();
    m.createActivationWindow();
    expect(m.getActivationWindow()).not.toBeNull();
  });

  it('closeActivationWindow closes and clears the reference', async () => {
    const m = await import('@/main/activation');
    m.createActivationWindow();
    m.closeActivationWindow();
    expect(m.getActivationWindow()).toBeNull();
  });

  it('init wires up ipc handlers', async () => {
    const m = await import('@/main/activation');
    m.init();
    expect(ipcHandlers['license:activated']).toBeDefined();
    expect(ipcHandlers['license:close']).toBeDefined();
    expect(ipcHandlers['license:deleted']).toBeDefined();
    expect(ipcHandlers['license:open-activation']).toBeDefined();
  });

  it('license:activated closes window, broadcasts license:changed and shows notification', async () => {
    const m = await import('@/main/activation');
    const other = new MockBrowserWindow({});
    m.createActivationWindow();
    m.init();
    await ipcHandlers['license:activated']();
    expect(m.getActivationWindow()).toBeNull();
    expect(other.webContents.send).toHaveBeenCalledWith('license:changed');
    expect(mockNotificationShow).toHaveBeenCalled();
  });

  it('license:close closes activation window', async () => {
    const m = await import('@/main/activation');
    m.createActivationWindow();
    m.init();
    ipcHandlers['license:close']();
    expect(m.getActivationWindow()).toBeNull();
  });

  it('license:deleted only broadcasts license:changed', async () => {
    const m = await import('@/main/activation');
    const other = new MockBrowserWindow({});
    m.init();
    ipcHandlers['license:deleted']();
    expect(other.webContents.send).toHaveBeenCalledWith('license:changed');
    expect(other.close).not.toHaveBeenCalled();
  });

  it('license:open-activation creates the activation window', async () => {
    const m = await import('@/main/activation');
    m.init();
    expect(m.getActivationWindow()).toBeNull();
    ipcHandlers['license:open-activation']();
    expect(m.getActivationWindow()).not.toBeNull();
  });

  describe('window lifecycle', () => {
    it('did-finish-load sends load event', async () => {
      const m = await import('@/main/activation');
      m.createActivationWindow();
      const win = browserWindowInstances[0];
      const handlers = win.webContentsHandlers['did-finish-load'] || [];
      handlers.forEach(cb => cb());
      expect(win.webContents.send).toHaveBeenCalledWith(
        'load',
        expect.objectContaining({ type: 'activation' })
      );
    });

    it('ready-to-show focuses and shows window', async () => {
      const m = await import('@/main/activation');
      m.createActivationWindow();
      const win = browserWindowInstances[0];
      const handlers = win.windowHandlers['ready-to-show'] || [];
      handlers.forEach(cb => cb());
      expect(win.show).toHaveBeenCalled();
      expect(win.focus).toHaveBeenCalled();
    });

    it('closed handler clears reference', async () => {
      const m = await import('@/main/activation');
      m.createActivationWindow();
      const win = browserWindowInstances[0];
      const handlers = win.windowHandlers['closed'] || [];
      handlers.forEach(cb => cb());
      expect(m.getActivationWindow()).toBeNull();
    });

    it('setWindowOpenHandler denies internal navigation', async () => {
      const m = await import('@/main/activation');
      const win = m.createActivationWindow();
      expect(win.webContents.setWindowOpenHandler).toHaveBeenCalled();
      const handler = browserWindowInstances[0].webContents.setWindowOpenHandler
        .mock.calls[0][0] as (args: { url: string }) => { action: string };
      const result = handler({ url: 'https://example.com' });
      expect(result.action).toBe('deny');
    });
  });
});
