import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { LicenseCache } from '@/types/license';

// Mock file system (sync functions)
const mockFs = {
  existsSync: vi.fn(),
  mkdirSync: vi.fn(),
  readFileSync: vi.fn(),
  writeFileSync: vi.fn(),
  unlinkSync: vi.fn(),
};

vi.mock('fs', () => ({
  default: mockFs,
  existsSync: mockFs.existsSync,
  mkdirSync: mockFs.mkdirSync,
  readFileSync: mockFs.readFileSync,
  writeFileSync: mockFs.writeFileSync,
  unlinkSync: mockFs.unlinkSync,
}));

// Mock file system (async functions)
const mockFsPromises = {
  readFile: vi.fn(() => Promise.resolve('')),
  writeFile: vi.fn(() => Promise.resolve()),
  mkdir: vi.fn(() => Promise.resolve()),
  unlink: vi.fn(() => Promise.resolve()),
};

vi.mock('fs/promises', () => ({
  default: mockFsPromises,
  readFile: mockFsPromises.readFile,
  writeFile: mockFsPromises.writeFile,
  mkdir: mockFsPromises.mkdir,
  unlink: mockFsPromises.unlink,
}));

// Mock license config
vi.mock('@/main/license/config', () => ({
  CONFIG_DIR: '/mock/config',
  LICENSE_FILE: '/mock/config/license.json',
}));

// Mock device fingerprint
const mockGenerateDeviceFingerprint = vi.fn(() => 'mock-fingerprint-123');
vi.mock('@/main/license/device', () => ({
  generateDeviceFingerprint: mockGenerateDeviceFingerprint,
}));

describe('License Cache', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    mockFs.existsSync.mockReturnValue(false);
    mockGenerateDeviceFingerprint.mockReturnValue('mock-fingerprint-123');
  });

  afterEach(() => {
    vi.resetModules();
  });

  describe('loadCachedLicense', () => {
    it('should return null when license file does not exist', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { loadCachedLicense } = await import('@/main/license/cache');
      const license = await loadCachedLicense();

      expect(license).toBeNull();
    });

    it('should load license from file when it exists', async () => {
      const mockLicense: LicenseCache = {
        licenseKey: 'TEST-KEY-123',
        email: 'test@example.com',
        deviceFingerprint: 'mock-fingerprint-123',
        isLifetime: true,
        expiresAt: null,
        maxVersion: null,
        lastValidated: new Date().toISOString(),
      };

      mockFs.existsSync.mockReturnValue(true);
      mockFsPromises.readFile.mockResolvedValue(JSON.stringify(mockLicense));

      const { loadCachedLicense } = await import('@/main/license/cache');
      const license = await loadCachedLicense();

      expect(license).toEqual(mockLicense);
      expect(mockFsPromises.readFile).toHaveBeenCalledWith(
        '/mock/config/license.json',
        'utf8'
      );
    });

    it('should return null on parse error', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFsPromises.readFile.mockResolvedValue('invalid json');

      const { loadCachedLicense } = await import('@/main/license/cache');
      const license = await loadCachedLicense();

      expect(license).toBeNull();
    });

    it('should return null on read error', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFsPromises.readFile.mockRejectedValue(new Error('Permission denied'));

      const { loadCachedLicense } = await import('@/main/license/cache');
      const license = await loadCachedLicense();

      expect(license).toBeNull();
    });
  });

  describe('saveLicenseCache', () => {
    it('should save license to file', async () => {
      const mockLicense: LicenseCache = {
        licenseKey: 'TEST-KEY-123',
        email: 'test@example.com',
        deviceFingerprint: 'mock-fingerprint-123',
        isLifetime: true,
        expiresAt: null,
        maxVersion: null,
        lastValidated: new Date().toISOString(),
      };

      mockFs.existsSync.mockReturnValue(true);

      const { saveLicenseCache } = await import('@/main/license/cache');
      await saveLicenseCache(mockLicense);

      expect(mockFsPromises.writeFile).toHaveBeenCalledWith(
        '/mock/config/license.json',
        JSON.stringify(mockLicense, null, 2)
      );
    });

    it('should create config directory if it does not exist', async () => {
      const mockLicense: LicenseCache = {
        licenseKey: 'TEST-KEY-123',
        email: 'test@example.com',
        deviceFingerprint: 'mock-fingerprint-123',
        isLifetime: false,
        expiresAt: '2025-12-31',
        maxVersion: '2.0.0',
        lastValidated: new Date().toISOString(),
      };

      mockFs.existsSync.mockReturnValue(false);

      const { saveLicenseCache } = await import('@/main/license/cache');
      await saveLicenseCache(mockLicense);

      expect(mockFs.mkdirSync).toHaveBeenCalledWith('/mock/config', {
        recursive: true,
      });
    });

    it('should handle write errors gracefully', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFsPromises.writeFile.mockRejectedValue(new Error('Disk full'));

      const mockLicense: LicenseCache = {
        licenseKey: 'TEST-KEY-123',
        email: 'test@example.com',
        deviceFingerprint: 'mock-fingerprint-123',
        isLifetime: true,
        expiresAt: null,
        maxVersion: null,
        lastValidated: new Date().toISOString(),
      };

      const { saveLicenseCache } = await import('@/main/license/cache');

      // Should not throw
      await expect(saveLicenseCache(mockLicense)).resolves.not.toThrow();
    });
  });

  describe('clearLicenseCache', () => {
    it('should delete license file', async () => {
      mockFs.existsSync.mockReturnValue(true);

      const { clearLicenseCache } = await import('@/main/license/cache');
      await clearLicenseCache();

      expect(mockFsPromises.unlink).toHaveBeenCalledWith(
        '/mock/config/license.json'
      );
    });

    it('should reset cached license state', async () => {
      mockFs.existsSync.mockReturnValue(true);

      const { clearLicenseCache, getCachedLicense, getLicenseStatus } =
        await import('@/main/license/cache');

      await clearLicenseCache();

      expect(getCachedLicense()).toBeNull();
      expect(getLicenseStatus()).toBe('not_activated');
    });

    it('should handle missing file gracefully', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { clearLicenseCache } = await import('@/main/license/cache');

      // Should not throw
      await expect(clearLicenseCache()).resolves.not.toThrow();
    });

    it('should handle deletion errors gracefully', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFsPromises.unlink.mockRejectedValue(new Error('Permission denied'));

      const { clearLicenseCache } = await import('@/main/license/cache');

      // Should not throw
      await expect(clearLicenseCache()).resolves.not.toThrow();
    });
  });

  describe('isOfflineCacheValid', () => {
    it('should return true for valid lifetime license', async () => {
      const mockLicense: LicenseCache = {
        licenseKey: 'TEST-KEY-123',
        email: 'test@example.com',
        deviceFingerprint: 'mock-fingerprint-123',
        isLifetime: true,
        expiresAt: null,
        maxVersion: null,
        lastValidated: new Date().toISOString(),
      };

      const { isOfflineCacheValid } = await import('@/main/license/cache');
      const valid = isOfflineCacheValid(mockLicense);

      expect(valid).toBe(true);
    });

    it('should return false when device fingerprint does not match', async () => {
      const mockLicense: LicenseCache = {
        licenseKey: 'TEST-KEY-123',
        email: 'test@example.com',
        deviceFingerprint: 'different-fingerprint',
        isLifetime: true,
        expiresAt: null,
        maxVersion: null,
        lastValidated: new Date().toISOString(),
      };

      const { isOfflineCacheValid } = await import('@/main/license/cache');
      const valid = isOfflineCacheValid(mockLicense);

      expect(valid).toBe(false);
    });

    it('should return true for non-expired time-limited license', async () => {
      const futureDate = new Date();
      futureDate.setFullYear(futureDate.getFullYear() + 1);

      const mockLicense: LicenseCache = {
        licenseKey: 'TEST-KEY-123',
        email: 'test@example.com',
        deviceFingerprint: 'mock-fingerprint-123',
        isLifetime: false,
        expiresAt: futureDate.toISOString(),
        maxVersion: null,
        lastValidated: new Date().toISOString(),
      };

      const { isOfflineCacheValid } = await import('@/main/license/cache');
      const valid = isOfflineCacheValid(mockLicense);

      expect(valid).toBe(true);
    });

    it('should return false for expired time-limited license', async () => {
      const pastDate = new Date();
      pastDate.setFullYear(pastDate.getFullYear() - 1);

      const mockLicense: LicenseCache = {
        licenseKey: 'TEST-KEY-123',
        email: 'test@example.com',
        deviceFingerprint: 'mock-fingerprint-123',
        isLifetime: false,
        expiresAt: pastDate.toISOString(),
        maxVersion: null,
        lastValidated: new Date().toISOString(),
      };

      const { isOfflineCacheValid } = await import('@/main/license/cache');
      const valid = isOfflineCacheValid(mockLicense);

      expect(valid).toBe(false);
    });

    it('should return true for non-lifetime license without expiration', async () => {
      const mockLicense: LicenseCache = {
        licenseKey: 'TEST-KEY-123',
        email: 'test@example.com',
        deviceFingerprint: 'mock-fingerprint-123',
        isLifetime: false,
        expiresAt: null,
        maxVersion: null,
        lastValidated: new Date().toISOString(),
      };

      const { isOfflineCacheValid } = await import('@/main/license/cache');
      const valid = isOfflineCacheValid(mockLicense);

      expect(valid).toBe(true);
    });
  });

  describe('getCachedLicense / setCachedLicense', () => {
    it('should get and set cached license', async () => {
      const mockLicense: LicenseCache = {
        licenseKey: 'TEST-KEY-123',
        email: 'test@example.com',
        deviceFingerprint: 'mock-fingerprint-123',
        isLifetime: true,
        expiresAt: null,
        maxVersion: null,
        lastValidated: new Date().toISOString(),
      };

      const { getCachedLicense, setCachedLicense } =
        await import('@/main/license/cache');

      setCachedLicense(mockLicense);
      const retrieved = getCachedLicense();

      expect(retrieved).toEqual(mockLicense);
    });

    it('should allow setting to null', async () => {
      const { getCachedLicense, setCachedLicense } =
        await import('@/main/license/cache');

      setCachedLicense(null);
      const retrieved = getCachedLicense();

      expect(retrieved).toBeNull();
    });
  });

  describe('getLicenseStatus / setLicenseStatus', () => {
    it('should get and set license status', async () => {
      const { getLicenseStatus, setLicenseStatus } =
        await import('@/main/license/cache');

      setLicenseStatus('valid');
      expect(getLicenseStatus()).toBe('valid');

      setLicenseStatus('invalid');
      expect(getLicenseStatus()).toBe('invalid');

      setLicenseStatus('offline_valid');
      expect(getLicenseStatus()).toBe('offline_valid');
    });

    it('should default to not_activated', async () => {
      const { getLicenseStatus } = await import('@/main/license/cache');

      expect(getLicenseStatus()).toBe('not_activated');
    });
  });
});
