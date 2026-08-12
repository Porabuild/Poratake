import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockDaemonCall = vi.fn();

vi.mock('@/main/daemon', () => ({
  daemon: {
    call: (...args: unknown[]) => mockDaemonCall(...args),
  },
}));

describe('Freeze Screen Module', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.resetModules();
  });

  describe('isSupported', () => {
    it('should return true on darwin platform', async () => {
      const originalPlatform = process.platform;
      Object.defineProperty(process, 'platform', { value: 'darwin' });

      const { isSupported } = await import('@/main/capture/freeze-screen');
      expect(isSupported()).toBe(true);

      Object.defineProperty(process, 'platform', { value: originalPlatform });
    });

    it('should return true on win32 platform', async () => {
      const originalPlatform = process.platform;
      Object.defineProperty(process, 'platform', { value: 'win32' });

      const { isSupported } = await import('@/main/capture/freeze-screen');
      expect(isSupported()).toBe(true);

      Object.defineProperty(process, 'platform', { value: originalPlatform });
    });
  });

  describe('freezeScreen', () => {
    it('should call daemon freeze-screen module and return true on success', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });

      const { freezeScreen } = await import('@/main/capture/freeze-screen');
      const result = await freezeScreen();

      expect(result).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith('freeze-screen', 'freeze', {
        watchSpaceKey: false,
      });
    });

    it('should pass watchSpaceKey param to daemon', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });

      const { freezeScreen } = await import('@/main/capture/freeze-screen');
      await freezeScreen(true);

      expect(mockDaemonCall).toHaveBeenCalledWith('freeze-screen', 'freeze', {
        watchSpaceKey: true,
      });
    });

    it('should call daemon on win32 platform', async () => {
      const originalPlatform = process.platform;
      Object.defineProperty(process, 'platform', { value: 'win32' });
      mockDaemonCall.mockResolvedValue({ success: true });

      const { freezeScreen } = await import('@/main/capture/freeze-screen');
      const result = await freezeScreen();

      expect(result).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith('freeze-screen', 'freeze', {
        watchSpaceKey: false,
      });

      Object.defineProperty(process, 'platform', { value: originalPlatform });
    });

    it('should return false and log error on daemon call failure', async () => {
      mockDaemonCall.mockRejectedValue(new Error('Daemon error'));
      const consoleSpy = vi
        .spyOn(console, 'error')
        .mockImplementation(() => {});

      const { freezeScreen } = await import('@/main/capture/freeze-screen');
      const result = await freezeScreen();

      expect(result).toBe(false);
      expect(consoleSpy).toHaveBeenCalledWith(
        'Failed to freeze screen:',
        expect.any(Error)
      );
      consoleSpy.mockRestore();
    });

    it('should set isFrozen to true on success', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });

      const { freezeScreen, isScreenFrozen } =
        await import('@/main/capture/freeze-screen');
      await freezeScreen();

      expect(isScreenFrozen()).toBe(true);
    });
  });

  describe('releaseScreen', () => {
    it('should call daemon freeze-screen release and return true on success', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });

      const { freezeScreen, releaseScreen } =
        await import('@/main/capture/freeze-screen');
      await freezeScreen();
      const result = await releaseScreen();

      expect(result).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith('freeze-screen', 'release');
    });

    it('should return false when not currently frozen', async () => {
      const { releaseScreen } = await import('@/main/capture/freeze-screen');
      const result = await releaseScreen();

      expect(result).toBe(false);
      expect(mockDaemonCall).not.toHaveBeenCalled();
    });

    it('should return false on unsupported platform', async () => {
      const originalPlatform = process.platform;
      Object.defineProperty(process, 'platform', { value: 'linux' });

      const { releaseScreen } = await import('@/main/capture/freeze-screen');
      const result = await releaseScreen();

      expect(result).toBe(false);
      expect(mockDaemonCall).not.toHaveBeenCalled();

      Object.defineProperty(process, 'platform', { value: originalPlatform });
    });

    it('should return false and log error on daemon call failure', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });

      const { freezeScreen, releaseScreen } =
        await import('@/main/capture/freeze-screen');
      await freezeScreen();

      mockDaemonCall.mockRejectedValue(new Error('Daemon error'));
      const consoleSpy = vi
        .spyOn(console, 'error')
        .mockImplementation(() => {});

      const result = await releaseScreen();

      expect(result).toBe(false);
      expect(consoleSpy).toHaveBeenCalledWith(
        'Failed to release screen:',
        expect.any(Error)
      );
      consoleSpy.mockRestore();
    });

    it('should set isFrozen to false on success', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });

      const { freezeScreen, releaseScreen, isScreenFrozen } =
        await import('@/main/capture/freeze-screen');

      await freezeScreen();
      expect(isScreenFrozen()).toBe(true);

      await releaseScreen();
      expect(isScreenFrozen()).toBe(false);
    });
  });

  describe('isScreenFrozen', () => {
    it('should return false initially', async () => {
      const { isScreenFrozen } = await import('@/main/capture/freeze-screen');
      expect(isScreenFrozen()).toBe(false);
    });

    it('should return true after freezeScreen succeeds', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });

      const { freezeScreen, isScreenFrozen } =
        await import('@/main/capture/freeze-screen');

      await freezeScreen();
      expect(isScreenFrozen()).toBe(true);
    });

    it('should return false after releaseScreen succeeds', async () => {
      mockDaemonCall.mockResolvedValue({ success: true });

      const { freezeScreen, releaseScreen, isScreenFrozen } =
        await import('@/main/capture/freeze-screen');

      await freezeScreen();
      expect(isScreenFrozen()).toBe(true);

      await releaseScreen();
      expect(isScreenFrozen()).toBe(false);
    });

    it('should remain false if freezeScreen fails', async () => {
      mockDaemonCall.mockRejectedValue(new Error('Daemon error'));
      vi.spyOn(console, 'error').mockImplementation(() => {});

      const { freezeScreen, isScreenFrozen } =
        await import('@/main/capture/freeze-screen');

      await freezeScreen();
      expect(isScreenFrozen()).toBe(false);
    });
  });
});
