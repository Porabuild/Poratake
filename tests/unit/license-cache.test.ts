import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockExistsSync = vi.fn();
const mockMkdirSync = vi.fn();
const mockReadFile = vi.fn();
const mockWriteFile = vi.fn();
const mockUnlink = vi.fn();
const mockGenerateDeviceFingerprint = vi.fn(() => 'device-1');

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    mkdirSync: (...a: unknown[]) => mockMkdirSync(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  mkdirSync: (...a: unknown[]) => mockMkdirSync(...a),
}));

vi.mock('fs/promises', () => ({
  default: {
    readFile: (...a: unknown[]) => mockReadFile(...a),
    writeFile: (...a: unknown[]) => mockWriteFile(...a),
    unlink: (...a: unknown[]) => mockUnlink(...a),
  },
  readFile: (...a: unknown[]) => mockReadFile(...a),
  writeFile: (...a: unknown[]) => mockWriteFile(...a),
  unlink: (...a: unknown[]) => mockUnlink(...a),
}));

vi.mock('@/main/license/config.ts', () => ({
  CONFIG_DIR: '/cfg',
  LICENSE_FILE: '/cfg/license.json',
}));

vi.mock('@/main/license/device.ts', () => ({
  generateDeviceFingerprint: () => mockGenerateDeviceFingerprint(),
}));

describe('license cache', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  describe('loadCachedLicense', () => {
    it('returns null when file missing', async () => {
      mockExistsSync.mockReturnValue(false);
      const { loadCachedLicense } = await import('@/main/license/cache');
      expect(await loadCachedLicense()).toBeNull();
    });

    it('returns parsed license when file exists', async () => {
      mockExistsSync.mockReturnValue(true);
      mockReadFile.mockResolvedValue('{"key":"abc"}');
      const { loadCachedLicense } = await import('@/main/license/cache');
      const result = await loadCachedLicense();
      expect(result).toEqual({ key: 'abc' });
    });

    it('returns null on parse error', async () => {
      mockExistsSync.mockReturnValue(true);
      mockReadFile.mockResolvedValue('bad json');
      const { loadCachedLicense } = await import('@/main/license/cache');
      expect(await loadCachedLicense()).toBeNull();
    });
  });

  describe('saveLicenseCache', () => {
    it('creates config dir if missing', async () => {
      mockExistsSync.mockReturnValue(false);
      const { saveLicenseCache } = await import('@/main/license/cache');
      await saveLicenseCache({ key: 'x' } as never);
      expect(mockMkdirSync).toHaveBeenCalledWith('/cfg', { recursive: true });
      expect(mockWriteFile).toHaveBeenCalled();
    });

    it('writes when dir exists', async () => {
      mockExistsSync.mockReturnValue(true);
      const { saveLicenseCache } = await import('@/main/license/cache');
      await saveLicenseCache({ key: 'x' } as never);
      expect(mockMkdirSync).not.toHaveBeenCalled();
      expect(mockWriteFile).toHaveBeenCalled();
    });

    it('swallows write errors', async () => {
      mockExistsSync.mockReturnValue(true);
      mockWriteFile.mockRejectedValue(new Error('disk full'));
      const { saveLicenseCache } = await import('@/main/license/cache');
      await expect(
        saveLicenseCache({ key: 'x' } as never)
      ).resolves.toBeUndefined();
    });
  });

  describe('clearLicenseCache', () => {
    it('clearLicenseCache unlinks file when present', async () => {
      mockExistsSync.mockReturnValue(true);
      const { clearLicenseCache } = await import('@/main/license/cache');
      await clearLicenseCache();
      expect(mockUnlink).toHaveBeenCalled();
    });

    it('clearLicenseCache skips when missing', async () => {
      mockExistsSync.mockReturnValue(false);
      const { clearLicenseCache } = await import('@/main/license/cache');
      await clearLicenseCache();
      expect(mockUnlink).not.toHaveBeenCalled();
    });

    it('clearLicenseCache swallows errors', async () => {
      mockExistsSync.mockReturnValue(true);
      mockUnlink.mockRejectedValue(new Error('locked'));
      const { clearLicenseCache } = await import('@/main/license/cache');
      await expect(clearLicenseCache()).resolves.toBeUndefined();
    });
  });

  describe('isOfflineCacheValid', () => {
    it('returns false for mismatched fingerprint', async () => {
      const { isOfflineCacheValid } = await import('@/main/license/cache');
      expect(
        isOfflineCacheValid({
          deviceFingerprint: 'other',
          isLifetime: true,
        } as never)
      ).toBe(false);
    });

    it('returns true for lifetime license', async () => {
      const { isOfflineCacheValid } = await import('@/main/license/cache');
      expect(
        isOfflineCacheValid({
          deviceFingerprint: 'device-1',
          isLifetime: true,
        } as never)
      ).toBe(true);
    });

    it('returns false when expired', async () => {
      const { isOfflineCacheValid } = await import('@/main/license/cache');
      expect(
        isOfflineCacheValid({
          deviceFingerprint: 'device-1',
          isLifetime: false,
          expiresAt: '2000-01-01T00:00:00Z',
        } as never)
      ).toBe(false);
    });

    it('returns true when not expired', async () => {
      const { isOfflineCacheValid } = await import('@/main/license/cache');
      expect(
        isOfflineCacheValid({
          deviceFingerprint: 'device-1',
          isLifetime: false,
          expiresAt: '2099-01-01T00:00:00Z',
        } as never)
      ).toBe(true);
    });

    it('returns true when no expiry', async () => {
      const { isOfflineCacheValid } = await import('@/main/license/cache');
      expect(
        isOfflineCacheValid({
          deviceFingerprint: 'device-1',
          isLifetime: false,
        } as never)
      ).toBe(true);
    });
  });

  describe('getters/setters', () => {
    it('cached license round-trip', async () => {
      const m = await import('@/main/license/cache');
      expect(m.getCachedLicense()).toBeNull();
      m.setCachedLicense({ key: 'x' } as never);
      expect(m.getCachedLicense()).toEqual({ key: 'x' });
      m.setCachedLicense(null);
      expect(m.getCachedLicense()).toBeNull();
    });

    it('license status round-trip', async () => {
      const m = await import('@/main/license/cache');
      expect(m.getLicenseStatus()).toBe('not_activated');
      m.setLicenseStatus('active');
      expect(m.getLicenseStatus()).toBe('active');
    });
  });
});
