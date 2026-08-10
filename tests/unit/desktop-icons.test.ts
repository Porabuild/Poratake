import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockDaemonCall = vi.fn();
const mockIsTrustedAccessibilityClient = vi.fn();

vi.mock('electron', () => ({
  systemPreferences: {
    isTrustedAccessibilityClient: (...args: unknown[]) =>
      mockIsTrustedAccessibilityClient(...args),
  },
}));

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...args: unknown[]) => mockDaemonCall(...args) },
}));

describe('desktop-icons', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  describe('isSupported', () => {
    it('returns true on darwin', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      const { isSupported } = await import('@/main/capture/desktop-icons');
      expect(isSupported()).toBe(true);
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('returns true on win32', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'win32' });
      const { isSupported } = await import('@/main/capture/desktop-icons');
      expect(isSupported()).toBe(true);
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('returns false on linux', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'linux' });
      const { isSupported } = await import('@/main/capture/desktop-icons');
      expect(isSupported()).toBe(false);
      Object.defineProperty(process, 'platform', { value: original });
    });
  });

  describe('checkAccessibilityPermission', () => {
    it('returns true on Windows without querying system preferences', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'win32' });
      const { checkAccessibilityPermission } =
        await import('@/main/capture/desktop-icons');
      expect(checkAccessibilityPermission(true)).toBe(true);
      expect(mockIsTrustedAccessibilityClient).not.toHaveBeenCalled();
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('passes prompt flag through', async () => {
      mockIsTrustedAccessibilityClient.mockReturnValue(true);
      const { checkAccessibilityPermission } =
        await import('@/main/capture/desktop-icons');
      expect(checkAccessibilityPermission(true)).toBe(true);
      expect(mockIsTrustedAccessibilityClient).toHaveBeenCalledWith(true);
    });

    it('defaults prompt to false', async () => {
      mockIsTrustedAccessibilityClient.mockReturnValue(false);
      const { checkAccessibilityPermission } =
        await import('@/main/capture/desktop-icons');
      expect(checkAccessibilityPermission()).toBe(false);
      expect(mockIsTrustedAccessibilityClient).toHaveBeenCalledWith(false);
    });
  });

  describe('hideDesktopIcons / showDesktopIcons', () => {
    it('hides desktop icons via daemon on darwin', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      mockDaemonCall.mockResolvedValue({});
      const { hideDesktopIcons, areDesktopIconsHidden } =
        await import('@/main/capture/desktop-icons');
      const result = await hideDesktopIcons('capture');
      expect(result).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith('desktop-helper', 'hide');
      expect(areDesktopIconsHidden()).toBe(true);
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('hides desktop icons via daemon on win32', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'win32' });
      mockDaemonCall.mockResolvedValue({});
      const { hideDesktopIcons } = await import('@/main/capture/desktop-icons');
      expect(await hideDesktopIcons('capture')).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith('desktop-helper', 'hide');
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('returns false on unsupported platforms without calling daemon', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'linux' });
      const { hideDesktopIcons } = await import('@/main/capture/desktop-icons');
      expect(await hideDesktopIcons('menu')).toBe(false);
      expect(mockDaemonCall).not.toHaveBeenCalled();
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('does not re-hide when already hidden', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      mockDaemonCall.mockResolvedValue({});
      const m = await import('@/main/capture/desktop-icons');
      await m.hideDesktopIcons('menu');
      mockDaemonCall.mockClear();
      const second = await m.hideDesktopIcons('capture');
      expect(second).toBe(true);
      expect(mockDaemonCall).not.toHaveBeenCalled();
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('returns false when daemon hide fails', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      const { hideDesktopIcons } = await import('@/main/capture/desktop-icons');
      expect(await hideDesktopIcons('menu')).toBe(false);
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('only shows when no other hide reasons remain', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      mockDaemonCall.mockResolvedValue({});
      const m = await import('@/main/capture/desktop-icons');
      await m.hideDesktopIcons('menu');
      await m.hideDesktopIcons('capture');
      mockDaemonCall.mockClear();

      const intermediate = await m.showDesktopIcons('capture');
      expect(intermediate).toBe(true);
      expect(mockDaemonCall).not.toHaveBeenCalled();

      const final = await m.showDesktopIcons('menu');
      expect(final).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith('desktop-helper', 'show');
      expect(m.areDesktopIconsHidden()).toBe(false);
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('keeps icons hidden until every active capture releases them', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      mockDaemonCall.mockResolvedValue({});
      const m = await import('@/main/capture/desktop-icons');
      await m.hideDesktopIcons('capture');
      await m.hideDesktopIcons('capture');
      mockDaemonCall.mockClear();

      expect(await m.showDesktopIcons('capture')).toBe(true);
      expect(mockDaemonCall).not.toHaveBeenCalled();
      expect(m.areDesktopIconsHidden()).toBe(true);

      expect(await m.showDesktopIcons('capture')).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith('desktop-helper', 'show');
      expect(m.areDesktopIconsHidden()).toBe(false);
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('system reset clears all reasons and shows', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      mockDaemonCall.mockResolvedValue({});
      const m = await import('@/main/capture/desktop-icons');
      await m.hideDesktopIcons('menu');
      await m.hideDesktopIcons('capture');
      mockDaemonCall.mockClear();
      const result = await m.showDesktopIcons('system');
      expect(result).toBe(true);
      expect(mockDaemonCall).toHaveBeenCalledWith('desktop-helper', 'show');
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('show is a no-op when already shown', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      const { showDesktopIcons } = await import('@/main/capture/desktop-icons');
      const result = await showDesktopIcons('system');
      expect(result).toBe(true);
      expect(mockDaemonCall).not.toHaveBeenCalled();
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('returns false when daemon show fails', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'darwin' });
      mockDaemonCall.mockResolvedValueOnce({});
      const m = await import('@/main/capture/desktop-icons');
      await m.hideDesktopIcons('menu');
      mockDaemonCall.mockRejectedValueOnce(new Error('boom'));
      expect(await m.showDesktopIcons('menu')).toBe(false);
      Object.defineProperty(process, 'platform', { value: original });
    });

    it('show on unsupported platforms returns false', async () => {
      const original = process.platform;
      Object.defineProperty(process, 'platform', { value: 'linux' });
      const { showDesktopIcons } = await import('@/main/capture/desktop-icons');
      expect(await showDesktopIcons('menu')).toBe(false);
      Object.defineProperty(process, 'platform', { value: original });
    });
  });
});
