import { beforeEach, describe, expect, it, vi } from 'vitest';

const exposeInMainWorld = vi.fn();
const on = vi.fn();
const off = vi.fn();
const send = vi.fn();
const invoke = vi.fn();

vi.mock('electron', () => ({
  contextBridge: { exposeInMainWorld },
  ipcRenderer: { on, off, send, invoke },
}));

interface ExposedIpc {
  on: (channel: string, listener: (...args: unknown[]) => void) => () => void;
}

describe('preload IPC bridge', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('removes the exact listener registered with Electron', async () => {
    await import('@/preload/preload');

    const exposed = exposeInMainWorld.mock.calls.find(
      ([key]) => key === 'ipcRenderer'
    )?.[1] as ExposedIpc;
    const listener = vi.fn();
    const unsubscribe = exposed.on('update:status-changed', listener);
    const wrappedListener = on.mock.calls[0][1];

    expect(wrappedListener).not.toBe(listener);

    unsubscribe();

    expect(off).toHaveBeenCalledWith('update:status-changed', wrappedListener);
  });
});
