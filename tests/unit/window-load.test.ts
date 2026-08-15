import { describe, it, expect, vi, beforeEach } from 'vitest';

const ipcHandlers: Record<string, (...args: unknown[]) => unknown> = {};

vi.mock('electron', () => ({
  ipcMain: {
    handle: vi.fn((channel: string, cb: (...args: unknown[]) => unknown) => {
      ipcHandlers[channel] = cb;
    }),
  },
}));

function createWebContents(id: number) {
  const destroyedHandlers: Array<() => void> = [];
  return {
    id,
    send: vi.fn(),
    once: vi.fn((event: string, cb: () => void) => {
      if (event === 'destroyed') destroyedHandlers.push(cb);
    }),
    destroy: () => destroyedHandlers.forEach(cb => cb()),
  };
}

describe('window-load', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    Object.keys(ipcHandlers).forEach(key => delete ipcHandlers[key]);
  });

  it('sends and retains the load payload', async () => {
    const { sendWindowLoad, initWindowLoad } =
      await import('@/main/utils/window-load');
    initWindowLoad();

    const webContents = createWebContents(1);
    const payload = { type: 'onboarding', params: {} };
    sendWindowLoad(webContents as never, payload);

    expect(webContents.send).toHaveBeenCalledWith('load', payload);

    const result = ipcHandlers['window:get-load-data']({
      sender: webContents,
    });
    expect(result).toEqual(payload);
  });

  it('returns null for a window without a stored payload', async () => {
    const { initWindowLoad } = await import('@/main/utils/window-load');
    initWindowLoad();

    const result = ipcHandlers['window:get-load-data']({
      sender: { id: 42 },
    });
    expect(result).toBeNull();
  });

  it('overwrites the payload on resend', async () => {
    const { sendWindowLoad, initWindowLoad } =
      await import('@/main/utils/window-load');
    initWindowLoad();

    const webContents = createWebContents(2);
    sendWindowLoad(webContents as never, {
      type: 'settings',
      params: { a: 1 },
    });
    sendWindowLoad(webContents as never, {
      type: 'settings',
      params: { a: 2 },
    });

    const result = ipcHandlers['window:get-load-data']({
      sender: webContents,
    });
    expect(result).toEqual({ type: 'settings', params: { a: 2 } });
  });

  it('drops the payload when the webContents is destroyed', async () => {
    const { sendWindowLoad, initWindowLoad } =
      await import('@/main/utils/window-load');
    initWindowLoad();

    const webContents = createWebContents(3);
    sendWindowLoad(webContents as never, { type: 'pin', params: {} });
    webContents.destroy();

    const result = ipcHandlers['window:get-load-data']({
      sender: webContents,
    });
    expect(result).toBeNull();
  });

  it('registers the destroyed cleanup only once per webContents', async () => {
    const { sendWindowLoad } = await import('@/main/utils/window-load');

    const webContents = createWebContents(4);
    sendWindowLoad(webContents as never, { type: 'pin', params: {} });
    sendWindowLoad(webContents as never, { type: 'pin', params: {} });

    expect(webContents.once).toHaveBeenCalledTimes(1);
  });
});
