import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDaemonCall = vi.fn();

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...args: unknown[]) => mockDaemonCall(...args) },
}));

describe('window-selector', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('selectWindow proxies daemon result', async () => {
    mockDaemonCall.mockResolvedValue({
      status: 'selected',
      windowId: 42,
      bounds: { x: 0, y: 0, width: 100, height: 100 },
    });
    const { selectWindow } = await import('@/main/capture/window-selector');
    const result = await selectWindow();
    expect(result.status).toBe('selected');
    expect(result.windowId).toBe(42);
    expect(mockDaemonCall).toHaveBeenCalledWith('window-selector', 'select');
  });

  it('listWindows returns the daemon window list', async () => {
    mockDaemonCall.mockResolvedValue({
      windows: [
        {
          windowId: 7,
          title: 'Window',
          ownerName: 'app',
          ownerPid: 42,
          bounds: { x: 10, y: 20, width: 300, height: 200 },
        },
      ],
    });
    const { listWindows } = await import('@/main/capture/window-selector');

    const result = await listWindows();

    expect(mockDaemonCall).toHaveBeenCalledWith('window-selector', 'list');
    expect(result).toHaveLength(1);
    expect(result[0].windowId).toBe(7);
  });

  it('listWindows defaults to an empty list', async () => {
    mockDaemonCall.mockResolvedValue({});
    const { listWindows } = await import('@/main/capture/window-selector');

    expect(await listWindows()).toEqual([]);
  });

  it('throws when invoked while already selecting', async () => {
    let resolveDaemon: (val: unknown) => void = () => {};
    mockDaemonCall.mockImplementation(
      () =>
        new Promise(res => {
          resolveDaemon = res;
        })
    );
    const { selectWindow } = await import('@/main/capture/window-selector');
    const first = selectWindow();
    await expect(selectWindow()).rejects.toThrow(
      'Window selector is already active'
    );
    resolveDaemon({ status: 'cancelled' });
    await first;
  });

  it('resets selecting flag after daemon error', async () => {
    mockDaemonCall.mockRejectedValueOnce(new Error('fail'));
    const { selectWindow } = await import('@/main/capture/window-selector');
    await expect(selectWindow()).rejects.toThrow('fail');
    mockDaemonCall.mockResolvedValueOnce({ status: 'selected' });
    const result = await selectWindow();
    expect(result.status).toBe('selected');
  });

  describe('killWindowSelector', () => {
    it('does nothing when not selecting', async () => {
      const { killWindowSelector } =
        await import('@/main/capture/window-selector');
      killWindowSelector();
      expect(mockDaemonCall).not.toHaveBeenCalled();
    });

    it('cancels when selecting', async () => {
      let resolveDaemon: (val: unknown) => void = () => {};
      mockDaemonCall.mockImplementationOnce(
        () =>
          new Promise(res => {
            resolveDaemon = res;
          })
      );
      const m = await import('@/main/capture/window-selector');
      const pending = m.selectWindow();
      mockDaemonCall.mockResolvedValueOnce({});
      m.killWindowSelector();
      expect(mockDaemonCall).toHaveBeenCalledWith('window-selector', 'cancel');
      resolveDaemon({ status: 'cancelled' });
      await pending;
    });
  });
});
