import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock Electron app
const mockApp = {
  getVersion: vi.fn(() => '1.0.0'),
  isPackaged: false,
};

vi.mock('electron', () => ({
  app: mockApp,
}));

// Mock env module
vi.mock('@/main/utils/env', () => ({
  isDev: false,
  isProduction: true,
  getAppVersion: () => mockApp.getVersion(),
}));

// Mock cache module
const mockGetLicenseStatus = vi.fn(() => 'valid');

vi.mock('@/main/license/cache', () => ({
  getLicenseStatus: mockGetLicenseStatus,
}));

describe('License Validation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockApp.getVersion.mockReturnValue('1.0.0');
    mockGetLicenseStatus.mockReturnValue('valid');
  });

  describe('compareVersions', () => {
    it('should return -1 when first version is lower', async () => {
      const { compareVersions } = await import('@/main/license/validation');
      expect(compareVersions('1.0.0', '1.0.1')).toBe(-1);
      expect(compareVersions('1.0.0', '2.0.0')).toBe(-1);
      expect(compareVersions('0.9.9', '1.0.0')).toBe(-1);
    });

    it('should return 1 when first version is higher', async () => {
      const { compareVersions } = await import('@/main/license/validation');
      expect(compareVersions('1.0.1', '1.0.0')).toBe(1);
      expect(compareVersions('2.0.0', '1.0.0')).toBe(1);
      expect(compareVersions('1.0.0', '0.9.9')).toBe(1);
    });

    it('should return 0 when versions are equal', async () => {
      const { compareVersions } = await import('@/main/license/validation');
      expect(compareVersions('1.0.0', '1.0.0')).toBe(0);
      expect(compareVersions('2.5.3', '2.5.3')).toBe(0);
      expect(compareVersions('0.24.2', '0.24.2')).toBe(0);
    });

    it('should handle versions with different number of parts', async () => {
      const { compareVersions } = await import('@/main/license/validation');
      expect(compareVersions('1.0', '1.0.0')).toBe(0);
      expect(compareVersions('1.0.0', '1.0')).toBe(0);
      expect(compareVersions('1.0', '1.0.1')).toBe(-1);
      expect(compareVersions('1.0.1', '1.0')).toBe(1);
    });

    it('should handle major version differences', async () => {
      const { compareVersions } = await import('@/main/license/validation');
      expect(compareVersions('2.0.0', '1.9.9')).toBe(1);
      expect(compareVersions('1.9.9', '2.0.0')).toBe(-1);
    });

    it('should handle minor version differences', async () => {
      const { compareVersions } = await import('@/main/license/validation');
      expect(compareVersions('1.5.0', '1.4.9')).toBe(1);
      expect(compareVersions('1.4.9', '1.5.0')).toBe(-1);
    });

    it('should handle patch version differences', async () => {
      const { compareVersions } = await import('@/main/license/validation');
      expect(compareVersions('1.0.5', '1.0.4')).toBe(1);
      expect(compareVersions('1.0.4', '1.0.5')).toBe(-1);
    });
  });

  describe('isVersionEntitled', () => {
    beforeEach(() => {
      mockApp.getVersion.mockReturnValue('1.5.0');
    });

    it('should return true when maxVersion is null', async () => {
      const { isVersionEntitled } = await import('@/main/license/validation');
      expect(isVersionEntitled(null)).toBe(true);
    });

    it('should return true when current version equals maxVersion', async () => {
      const { isVersionEntitled } = await import('@/main/license/validation');
      expect(isVersionEntitled('1.5.0')).toBe(true);
    });

    it('should return true when current version is lower than maxVersion', async () => {
      const { isVersionEntitled } = await import('@/main/license/validation');
      expect(isVersionEntitled('1.6.0')).toBe(true);
      expect(isVersionEntitled('2.0.0')).toBe(true);
    });

    it('should return false when current version exceeds maxVersion', async () => {
      const { isVersionEntitled } = await import('@/main/license/validation');
      expect(isVersionEntitled('1.4.0')).toBe(false);
      expect(isVersionEntitled('1.0.0')).toBe(false);
      expect(isVersionEntitled('0.9.0')).toBe(false);
    });

    it('should handle edge cases', async () => {
      const { isVersionEntitled } = await import('@/main/license/validation');

      mockApp.getVersion.mockReturnValue('2.0.0');
      expect(isVersionEntitled('1.9.9')).toBe(false);

      mockApp.getVersion.mockReturnValue('1.0.0');
      expect(isVersionEntitled('1.0.0')).toBe(true);
    });
  });

  describe('isPro', () => {
    beforeEach(() => {
      mockApp.getVersion.mockReturnValue('1.0.0');
      mockGetLicenseStatus.mockReturnValue('valid');
    });

    afterEach(() => {
      vi.clearAllMocks();
    });

    it('should return true when license is valid', async () => {
      mockGetLicenseStatus.mockReturnValue('valid');
      const { isPro } = await import('@/main/license/validation');
      expect(isPro()).toBe(true);
    });

    it('should return true when license is offline but valid', async () => {
      mockGetLicenseStatus.mockReturnValue('offline_valid');
      const { isPro } = await import('@/main/license/validation');
      expect(isPro()).toBe(true);
    });

    it('should return false when license is not activated', async () => {
      mockGetLicenseStatus.mockReturnValue('not_activated');
      const { isPro } = await import('@/main/license/validation');
      expect(isPro()).toBe(false);
    });

    it('should return false when license is invalid', async () => {
      mockGetLicenseStatus.mockReturnValue('invalid');
      const { isPro } = await import('@/main/license/validation');
      expect(isPro()).toBe(false);
    });

    it('should return false when device mismatch', async () => {
      mockGetLicenseStatus.mockReturnValue('device_mismatch');
      const { isPro } = await import('@/main/license/validation');
      expect(isPro()).toBe(false);
    });

    it('should return false when offline expired', async () => {
      mockGetLicenseStatus.mockReturnValue('offline_expired');
      const { isPro } = await import('@/main/license/validation');
      expect(isPro()).toBe(false);
    });

    it('should return false when expired', async () => {
      mockGetLicenseStatus.mockReturnValue('expired');
      const { isPro } = await import('@/main/license/validation');
      expect(isPro()).toBe(false);
    });
  });

  describe('isFirstTimeActivation', () => {
    afterEach(() => {
      vi.clearAllMocks();
    });

    it('should return true when license status is not_activated', async () => {
      mockGetLicenseStatus.mockReturnValue('not_activated');
      const { isFirstTimeActivation } =
        await import('@/main/license/validation');
      expect(isFirstTimeActivation()).toBe(true);
    });

    it('should return false for other license statuses', async () => {
      const { isFirstTimeActivation } =
        await import('@/main/license/validation');

      mockGetLicenseStatus.mockReturnValue('valid');
      expect(isFirstTimeActivation()).toBe(false);

      mockGetLicenseStatus.mockReturnValue('invalid');
      expect(isFirstTimeActivation()).toBe(false);

      mockGetLicenseStatus.mockReturnValue('device_mismatch');
      expect(isFirstTimeActivation()).toBe(false);

      mockGetLicenseStatus.mockReturnValue('offline_valid');
      expect(isFirstTimeActivation()).toBe(false);
    });
  });
});
