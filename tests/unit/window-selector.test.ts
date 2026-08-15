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
});
