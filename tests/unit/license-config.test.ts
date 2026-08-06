import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

describe('License Config', () => {
  const originalEnv = process.env.NODE_TLS_REJECT_UNAUTHORIZED;

  beforeEach(() => {
    vi.resetModules();
    delete process.env.NODE_TLS_REJECT_UNAUTHORIZED;
  });

  afterEach(() => {
    vi.restoreAllMocks();
    if (originalEnv !== undefined) {
      process.env.NODE_TLS_REJECT_UNAUTHORIZED = originalEnv;
    } else {
      delete process.env.NODE_TLS_REJECT_UNAUTHORIZED;
    }
  });

  describe('CONFIG_DIR', () => {
    it('should be derived from getConfigDir', async () => {
      const mockConfigDir = '/mock/config/dir';
      vi.doMock('@/main/utils/paths', () => ({
        getConfigDir: () => mockConfigDir,
        getLicenseFilePath: () => '/mock/license/file',
        getTrialFilePath: () => '/mock/trial/file',
      }));
      vi.doMock('@/main/utils/env', () => ({
        isProduction: false,
      }));

      const { CONFIG_DIR } = await import('@/main/license/config');
      expect(CONFIG_DIR).toBe(mockConfigDir);
    });
  });

  describe('LICENSE_FILE', () => {
    it('should be derived from getLicenseFilePath', async () => {
      const mockLicenseFile = '/mock/license/file.json';
      vi.doMock('@/main/utils/paths', () => ({
        getConfigDir: () => '/mock/config',
        getLicenseFilePath: () => mockLicenseFile,
        getTrialFilePath: () => '/mock/trial',
      }));
      vi.doMock('@/main/utils/env', () => ({
        isProduction: false,
      }));

      const { LICENSE_FILE } = await import('@/main/license/config');
      expect(LICENSE_FILE).toBe(mockLicenseFile);
    });
  });

  describe('API_URL', () => {
    it('should return production URL when isProduction is true', async () => {
      vi.doMock('@/main/utils/paths', () => ({
        getConfigDir: () => '/mock/config',
        getLicenseFilePath: () => '/mock/license',
        getTrialFilePath: () => '/mock/trial',
      }));
      vi.doMock('@/main/utils/env', () => ({
        isProduction: true,
      }));

      const { API_URL } = await import('@/main/license/config');
      expect(API_URL).toBe('https://capty.app');
    });

    it('should return development URL when isProduction is false', async () => {
      vi.doMock('@/main/utils/paths', () => ({
        getConfigDir: () => '/mock/config',
        getLicenseFilePath: () => '/mock/license',
        getTrialFilePath: () => '/mock/trial',
      }));
      vi.doMock('@/main/utils/env', () => ({
        isProduction: false,
      }));

      const { API_URL } = await import('@/main/license/config');
      expect(API_URL).toBe('https://capty.test');
    });
  });

  describe('initSSLSettings', () => {
    it('should set NODE_TLS_REJECT_UNAUTHORIZED to 1 in production', async () => {
      vi.doMock('@/main/utils/paths', () => ({
        getConfigDir: () => '/mock/config',
        getLicenseFilePath: () => '/mock/license',
        getTrialFilePath: () => '/mock/trial',
      }));
      vi.doMock('@/main/utils/env', () => ({
        isProduction: true,
      }));

      const { initSSLSettings } = await import('@/main/license/config');
      initSSLSettings();

      expect(process.env.NODE_TLS_REJECT_UNAUTHORIZED).toBe('1');
    });

    it('should set NODE_TLS_REJECT_UNAUTHORIZED to 0 in development', async () => {
      vi.doMock('@/main/utils/paths', () => ({
        getConfigDir: () => '/mock/config',
        getLicenseFilePath: () => '/mock/license',
        getTrialFilePath: () => '/mock/trial',
      }));
      vi.doMock('@/main/utils/env', () => ({
        isProduction: false,
      }));

      const { initSSLSettings } = await import('@/main/license/config');
      initSSLSettings();

      expect(process.env.NODE_TLS_REJECT_UNAUTHORIZED).toBe('0');
    });

    it('should enforce SSL in production even if previously disabled', async () => {
      process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0';

      vi.doMock('@/main/utils/paths', () => ({
        getConfigDir: () => '/mock/config',
        getLicenseFilePath: () => '/mock/license',
        getTrialFilePath: () => '/mock/trial',
      }));
      vi.doMock('@/main/utils/env', () => ({
        isProduction: true,
      }));

      const { initSSLSettings } = await import('@/main/license/config');
      initSSLSettings();

      expect(process.env.NODE_TLS_REJECT_UNAUTHORIZED).toBe('1');
    });
  });
});
