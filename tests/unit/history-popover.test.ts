import { describe, it, expect, vi, beforeEach } from 'vitest';

type Handler = (...args: unknown[]) => unknown;
const ipcOn: Record<string, Handler> = {};
const browserWindows: MockBrowserWindow[] = [];

class MockBrowserWindow {
  windowHandlers: Record<string, ((...a: unknown[]) => unknown)[]> = {};
  webContents = {
    on: vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
      this.windowHandlers[`wc:${event}`] ??= [];
      this.windowHandlers[`wc:${event}`].push(cb);
    }),
    send: vi.fn(),
  };
  isDestroyedFlag = false;
  isVisibleFlag = false;

  loadURL = vi.fn();
  loadFile = vi.fn();
  show = vi.fn(() => {
    this.isVisibleFlag = true;
  });
  focus = vi.fn();
  close = vi.fn(() => {
    this.isVisibleFlag = false;
    (this.windowHandlers['closed'] || []).forEach(cb => cb());
  });
  destroy = vi.fn();
  setPosition = vi.fn();
  setVisibleOnAllWorkspaces = vi.fn();
  isDestroyed = vi.fn(() => this.isDestroyedFlag);
  isVisible = vi.fn(() => this.isVisibleFlag);
  on = vi.fn((event: string, cb: (...a: unknown[]) => unknown) => {
    this.windowHandlers[event] ??= [];
    this.windowHandlers[event].push(cb);
  });

  constructor(_opts: unknown) {
    void _opts;
    browserWindows.push(this);
  }
}

vi.mock('electron', () => ({
  BrowserWindow: MockBrowserWindow,
  app: { focus: vi.fn() },
  screen: {
    getPrimaryDisplay: vi.fn(() => ({
      workAreaSize: { width: 1920, height: 1080 },
    })),
  },
  ipcMain: {
    on: vi.fn((e: string, h: Handler) => {
      ipcOn[e] = h;
    }),
  },
}));

vi.mock('@/main/utils/env', () => ({
  isDev: true,
  devServerUrl: 'http://localhost:5173',
}));

describe('history popover', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    browserWindows.splice(0);
    Object.keys(ipcOn).forEach(k => delete ipcOn[k]);
  });

  it('preloadHistoryPopover creates a window', async () => {
    const { preloadHistoryPopover } = await import('@/main/history/popover');
    preloadHistoryPopover();
    expect(browserWindows.length).toBe(1);
  });

  it('preload is idempotent', async () => {
    const { preloadHistoryPopover } = await import('@/main/history/popover');
    preloadHistoryPopover();
    preloadHistoryPopover();
    expect(browserWindows.length).toBe(1);
  });

  it('showHistoryPopover shows the window', async () => {
    const m = await import('@/main/history/popover');
    m.showHistoryPopover();
    expect(browserWindows[0].show).toHaveBeenCalled();
    expect(m.isHistoryPopoverVisible()).toBe(true);
  });

  it('showHistoryPopover positions near tray bounds when supplied', async () => {
    const m = await import('@/main/history/popover');
    m.showHistoryPopover({ x: 1000, y: 0, width: 40, height: 40 });
    expect(browserWindows[0].setPosition).toHaveBeenCalledWith(820, 44);
  });

  it('closeHistoryPopover closes the window', async () => {
    const m = await import('@/main/history/popover');
    m.showHistoryPopover();
    m.closeHistoryPopover();
    expect(browserWindows[0].close).toHaveBeenCalled();
  });

  it('toggle opens when closed and closes when open', async () => {
    const m = await import('@/main/history/popover');
    m.toggleHistoryPopover();
    expect(browserWindows[0].show).toHaveBeenCalled();
    m.toggleHistoryPopover();
    expect(browserWindows[0].close).toHaveBeenCalled();
  });

  it('getHistoryPopover returns the current window', async () => {
    const m = await import('@/main/history/popover');
    expect(m.getHistoryPopover()).toBeNull();
    m.preloadHistoryPopover();
    expect(m.getHistoryPopover()).not.toBeNull();
  });

  it('isHistoryPopoverVisible returns false initially', async () => {
    const m = await import('@/main/history/popover');
    expect(m.isHistoryPopoverVisible()).toBe(false);
  });

  it('history:closePopover IPC closes the popover', async () => {
    const m = await import('@/main/history/popover');
    m.preloadHistoryPopover();
    expect(ipcOn['history:closePopover']).toBeDefined();
    ipcOn['history:closePopover']();
    expect(browserWindows[0].close).toHaveBeenCalled();
  });

  it('window blur closes popover', async () => {
    const m = await import('@/main/history/popover');
    m.showHistoryPopover();
    const win = browserWindows[0];
    win.close.mockClear();
    (win.windowHandlers['blur'] || []).forEach(cb => cb());
    expect(win.close).toHaveBeenCalled();
  });

  it('did-finish-load marks popover ready and sends refresh on show', async () => {
    const m = await import('@/main/history/popover');
    m.preloadHistoryPopover();
    const win = browserWindows[0];
    // Fire did-finish-load
    (win.windowHandlers['wc:did-finish-load'] || []).forEach(cb => cb());
    // Show triggers refresh send when ready
    m.showHistoryPopover();
    expect(win.webContents.send).toHaveBeenCalledWith('history:refresh');
  });

  it('closed handler clears popover reference', async () => {
    const m = await import('@/main/history/popover');
    m.preloadHistoryPopover();
    const win = browserWindows[0];
    expect(m.getHistoryPopover()).toBe(win);
    (win.windowHandlers['closed'] || []).forEach(cb => cb());
    expect(m.getHistoryPopover()).toBeNull();
  });
});
