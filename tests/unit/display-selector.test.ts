import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockDaemonCall = vi.fn();
const mockScreenToDipRect = vi.fn();
const mockGetDisplayMatching = vi.fn();

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...args: unknown[]) => mockDaemonCall(...args) },
}));

vi.mock('electron', () => ({
  screen: {
    screenToDipRect: (...args: unknown[]) => mockScreenToDipRect(...args),
    getDisplayMatching: (...args: unknown[]) => mockGetDisplayMatching(...args),
  },
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

  describe('displayFromSelection', () => {
    const originalPlatform = process.platform;

    afterEach(() => {
      Object.defineProperty(process, 'platform', { value: originalPlatform });
    });

    it('returns null without bounds or when cancelled', async () => {
      const { displayFromSelection } =
        await import('@/main/capture/display-selector');
      expect(displayFromSelection({ status: 'cancelled' })).toBeNull();
      expect(
        displayFromSelection({ status: 'selected', displayNumber: 1 })
      ).toBeNull();
    });

    it('maps physical bounds to DIP before matching on Windows', async () => {
      Object.defineProperty(process, 'platform', { value: 'win32' });
      vi.resetModules();
      const bounds = { x: 0, y: 0, width: 3840, height: 2160 };
      const dipBounds = { x: 0, y: 0, width: 1920, height: 1080 };
      const display = { id: 7 };
      mockScreenToDipRect.mockReturnValue(dipBounds);
      mockGetDisplayMatching.mockReturnValue(display);

      const { displayFromSelection } =
        await import('@/main/capture/display-selector');
      const result = displayFromSelection({ status: 'selected', bounds });

      expect(mockScreenToDipRect).toHaveBeenCalledWith(null, bounds);
      expect(mockGetDisplayMatching).toHaveBeenCalledWith(dipBounds);
      expect(result).toBe(display);
    });
  });
});
