import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDaemonCall = vi.fn();

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...args: unknown[]) => mockDaemonCall(...args) },
}));

describe('display-selector', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('selectDisplay returns daemon result', async () => {
    mockDaemonCall.mockResolvedValue({ status: 'selected', displayNumber: 1 });
    const { selectDisplay } = await import('@/main/capture/display-selector');
    const result = await selectDisplay();
    expect(result).toEqual({ status: 'selected', displayNumber: 1 });
    expect(mockDaemonCall).toHaveBeenCalledWith('display-selector', 'select');
  });

  it('throws when already selecting', async () => {
    let resolve: (val: unknown) => void = () => {};
    mockDaemonCall.mockImplementation(
      () =>
        new Promise(res => {
          resolve = res;
        })
    );
    const { selectDisplay } = await import('@/main/capture/display-selector');
    const first = selectDisplay();
    await expect(selectDisplay()).rejects.toThrow(
      'Display selector is already active'
    );
    resolve({ status: 'cancelled' });
    await first;
  });

  describe('killDisplaySelector', () => {
    it('does nothing when not selecting', async () => {
      const { killDisplaySelector } =
        await import('@/main/capture/display-selector');
      killDisplaySelector();
      expect(mockDaemonCall).not.toHaveBeenCalled();
    });

    it('cancels when in progress', async () => {
      let resolve: (val: unknown) => void = () => {};
      mockDaemonCall.mockImplementationOnce(
        () =>
          new Promise(res => {
            resolve = res;
          })
      );
      const m = await import('@/main/capture/display-selector');
      const pending = m.selectDisplay();
      mockDaemonCall.mockResolvedValueOnce({});
      m.killDisplaySelector();
      expect(mockDaemonCall).toHaveBeenCalledWith('display-selector', 'cancel');
      resolve({ status: 'cancelled' });
      await pending;
    });
  });
});
