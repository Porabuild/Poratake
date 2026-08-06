import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const mockApp = {
  getVersion: vi.fn(() => '1.0.0'),
  isPackaged: false,
};

vi.mock('electron', () => ({
  app: mockApp,
}));

const mockDeviceFunctions = {
  generateDeviceFingerprint: vi.fn(() => 'mock-fingerprint'),
  getDeviceName: vi.fn(() => 'Mock-Device'),
  getDevicePlatform: vi.fn(() => 'macOS'),
};

vi.mock('@/main/license/device', () => mockDeviceFunctions);

const mockCacheFunctions = {
  getCachedLicense: vi.fn(),
  setCachedLicense: vi.fn(),
  setLicenseStatus: vi.fn(),
  saveLicenseCache: vi.fn(),
  clearLicenseCache: vi.fn(),
  isOfflineCacheValid: vi.fn(),
};

vi.mock('@/main/license/cache', () => mockCacheFunctions);

vi.mock('@/main/license/validation', () => ({
  isVersionEntitled: vi.fn(() => true),
}));

vi.mock('@/main/license/config', () => ({
  API_URL: 'https://api.test.com',
}));

// Mock global fetch
const mockFetch = vi.fn();
vi.stubGlobal('fetch', mockFetch);

describe('License API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCacheFunctions.getCachedLicense.mockReturnValue(null);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('activateLicense', () => {
    it('should activate license successfully', async () => {
      const mockResponse = {
        valid: true,
        license: {
          expires_at: '2025-12-31',
          max_version: '2.0.0',
          is_lifetime: false,
        },
      };
      mockFetch.mockResolvedValueOnce({
        json: () => Promise.resolve(mockResponse),
      });

      const { activateLicense } = await import('@/main/license/api');
      const result = await activateLicense('test@example.com', 'LICENSE-KEY');

      expect(result).toEqual(mockResponse);
      expect(mockFetch).toHaveBeenCalledWith(
        'https://api.test.com/api/license/activate',
        expect.objectContaining({
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Accept: 'application/json',
          },
        })
      );
      expect(mockCacheFunctions.saveLicenseCache).toHaveBeenCalled();
      expect(mockCacheFunctions.setCachedLicense).toHaveBeenCalled();
      expect(mockCacheFunctions.setLicenseStatus).toHaveBeenCalledWith('valid');
    });

    it('should handle activation failure', async () => {
      const mockResponse = {
        valid: false,
        error: 'invalid_license',
        message: 'License key is invalid',
      };
      mockFetch.mockResolvedValueOnce({
        json: () => Promise.resolve(mockResponse),
      });

      const { activateLicense } = await import('@/main/license/api');
      const result = await activateLicense('test@example.com', 'INVALID-KEY');

      expect(result).toEqual(mockResponse);
      expect(mockCacheFunctions.saveLicenseCache).not.toHaveBeenCalled();
    });

    it('should handle network errors', async () => {
      mockFetch.mockRejectedValueOnce(new Error('Network error'));

      const { activateLicense } = await import('@/main/license/api');
      const result = await activateLicense('test@example.com', 'LICENSE-KEY');

      expect(result.valid).toBe(false);
      expect(result.error).toBe('network_error');
      expect(result.message).toContain('Unable to connect');
    });

    it('should send correct request body', async () => {
      mockFetch.mockResolvedValueOnce({
        json: () => Promise.resolve({ valid: false }),
      });

      const { activateLicense } = await import('@/main/license/api');
      await activateLicense('user@test.com', 'TEST-KEY-123');

      const [, options] = mockFetch.mock.calls[0];
      const body = JSON.parse(options.body);

      expect(body).toEqual({
        email: 'user@test.com',
        license_key: 'TEST-KEY-123',
        device_fingerprint: 'mock-fingerprint',
        device_name: 'Mock-Device',
        device_platform: 'macOS',
        app_version: '1.0.0',
      });
    });
  });

  describe('validateLicense', () => {
    it('should return not_activated when no cached license', async () => {
      mockCacheFunctions.getCachedLicense.mockReturnValue(null);

      const { validateLicense } = await import('@/main/license/api');
      const result = await validateLicense();

      expect(result).toEqual({ valid: false, error: 'not_activated' });
    });

    it('should validate license successfully', async () => {
      const cachedLicense = {
        licenseKey: 'CACHED-KEY',
        email: 'cached@test.com',
        expiresAt: '2025-12-31',
        maxVersion: '2.0.0',
        isLifetime: false,
        deviceFingerprint: 'old-fingerprint',
        lastValidated: '2024-01-01',
      };
      mockCacheFunctions.getCachedLicense.mockReturnValue(cachedLicense);

      const mockResponse = {
        valid: true,
        license: {
          expires_at: '2025-12-31',
          max_version: '2.0.0',
          is_lifetime: false,
          is_expired: false,
        },
      };
      mockFetch.mockResolvedValueOnce({
        json: () => Promise.resolve(mockResponse),
      });

      const { validateLicense } = await import('@/main/license/api');
      const result = await validateLicense();

      expect(result).toEqual(mockResponse);
      expect(mockCacheFunctions.saveLicenseCache).toHaveBeenCalled();
      expect(mockCacheFunctions.setLicenseStatus).toHaveBeenCalledWith('valid');
    });

    it('should set expired status when license is expired', async () => {
      const cachedLicense = {
        licenseKey: 'CACHED-KEY',
        email: 'cached@test.com',
        expiresAt: '2023-12-31',
        maxVersion: '1.0.0',
        isLifetime: false,
        deviceFingerprint: 'fp',
        lastValidated: '2023-01-01',
      };
      mockCacheFunctions.getCachedLicense.mockReturnValue(cachedLicense);

      const mockResponse = {
        valid: true,
        license: {
          expires_at: '2023-12-31',
          max_version: '1.0.0',
          is_lifetime: false,
          is_expired: true,
        },
      };
      mockFetch.mockResolvedValueOnce({
        json: () => Promise.resolve(mockResponse),
      });

      const { validateLicense } = await import('@/main/license/api');
      await validateLicense();

      expect(mockCacheFunctions.setLicenseStatus).toHaveBeenCalledWith(
        'expired'
      );
    });

    it('should set device_mismatch status when device not activated', async () => {
      const cachedLicense = {
        licenseKey: 'CACHED-KEY',
        email: 'test@test.com',
        expiresAt: '2025-12-31',
        maxVersion: '2.0.0',
        isLifetime: false,
        deviceFingerprint: 'fp',
        lastValidated: '2024-01-01',
      };
      mockCacheFunctions.getCachedLicense.mockReturnValue(cachedLicense);

      const mockResponse = {
        valid: false,
        error: 'device_not_activated',
      };
      mockFetch.mockResolvedValueOnce({
        json: () => Promise.resolve(mockResponse),
      });

      const { validateLicense } = await import('@/main/license/api');
      await validateLicense();

      expect(mockCacheFunctions.setLicenseStatus).toHaveBeenCalledWith(
        'device_mismatch'
      );
    });

    it('should set invalid status for other validation failures', async () => {
      const cachedLicense = {
        licenseKey: 'CACHED-KEY',
        email: 'test@test.com',
        expiresAt: '2025-12-31',
        maxVersion: '2.0.0',
        isLifetime: false,
        deviceFingerprint: 'fp',
        lastValidated: '2024-01-01',
      };
      mockCacheFunctions.getCachedLicense.mockReturnValue(cachedLicense);

      const mockResponse = {
        valid: false,
        error: 'license_revoked',
      };
      mockFetch.mockResolvedValueOnce({
        json: () => Promise.resolve(mockResponse),
      });

      const { validateLicense } = await import('@/main/license/api');
      await validateLicense();

      expect(mockCacheFunctions.setLicenseStatus).toHaveBeenCalledWith(
        'invalid'
      );
    });

    it('should use cached license when offline and cache is valid', async () => {
      const cachedLicense = {
        licenseKey: 'CACHED-KEY',
        email: 'test@test.com',
        expiresAt: '2025-12-31',
        maxVersion: '2.0.0',
        isLifetime: true,
        deviceFingerprint: 'fp',
        lastValidated: '2024-01-01',
      };
      mockCacheFunctions.getCachedLicense.mockReturnValue(cachedLicense);
      mockCacheFunctions.isOfflineCacheValid.mockReturnValue(true);
      mockFetch.mockRejectedValueOnce(new Error('Network error'));

      const { validateLicense } = await import('@/main/license/api');
      const result = await validateLicense();

      expect(result.valid).toBe(true);
      expect(mockCacheFunctions.setLicenseStatus).toHaveBeenCalledWith(
        'offline_valid'
      );
    });

    it('should return offline_cache_expired when offline and cache expired', async () => {
      const cachedLicense = {
        licenseKey: 'CACHED-KEY',
        email: 'test@test.com',
        expiresAt: '2023-12-31',
        maxVersion: '1.0.0',
        isLifetime: false,
        deviceFingerprint: 'fp',
        lastValidated: '2023-01-01',
      };
      mockCacheFunctions.getCachedLicense.mockReturnValue(cachedLicense);
      mockCacheFunctions.isOfflineCacheValid.mockReturnValue(false);
      mockFetch.mockRejectedValueOnce(new Error('Network error'));

      const { validateLicense } = await import('@/main/license/api');
      const result = await validateLicense();

      expect(result.valid).toBe(false);
      expect(result.error).toBe('offline_cache_expired');
      expect(mockCacheFunctions.setLicenseStatus).toHaveBeenCalledWith(
        'offline_expired'
      );
    });
  });

  describe('deactivateLicense', () => {
    it('should return true when no cached license', async () => {
      mockCacheFunctions.getCachedLicense.mockReturnValue(null);

      const { deactivateLicense } = await import('@/main/license/api');
      const result = await deactivateLicense();

      expect(result).toBe(true);
      expect(mockFetch).not.toHaveBeenCalled();
    });

    it('should deactivate license successfully', async () => {
      const cachedLicense = {
        licenseKey: 'CACHED-KEY',
        email: 'test@test.com',
      };
      mockCacheFunctions.getCachedLicense.mockReturnValue(cachedLicense);
      mockFetch.mockResolvedValueOnce({});

      const { deactivateLicense } = await import('@/main/license/api');
      const result = await deactivateLicense();

      expect(result).toBe(true);
      expect(mockFetch).toHaveBeenCalledWith(
        'https://api.test.com/api/license/deactivate',
        expect.objectContaining({
          method: 'POST',
        })
      );
      expect(mockCacheFunctions.clearLicenseCache).toHaveBeenCalled();
    });

    it('should clear cache even on network error', async () => {
      const cachedLicense = {
        licenseKey: 'CACHED-KEY',
        email: 'test@test.com',
      };
      mockCacheFunctions.getCachedLicense.mockReturnValue(cachedLicense);
      mockFetch.mockRejectedValueOnce(new Error('Network error'));

      const { deactivateLicense } = await import('@/main/license/api');
      const result = await deactivateLicense();

      expect(result).toBe(true);
      expect(mockCacheFunctions.clearLicenseCache).toHaveBeenCalled();
    });
  });

  describe('getCheckoutUrl', () => {
    it('should return the pricing URL', async () => {
      const { getCheckoutUrl } = await import('@/main/license/api');
      const result = getCheckoutUrl();

      expect(result).toBe('https://api.test.com/#pricing');
    });
  });
});
