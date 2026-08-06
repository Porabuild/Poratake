import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock electron module before importing the module under test
vi.mock('electron', () => ({
  app: {
    isPackaged: false,
    getVersion: () => '1.0.0',
  },
}));

describe('Environment Utilities', () => {
  let originalEnv: NodeJS.ProcessEnv;

  beforeEach(() => {
    // Save original environment
    originalEnv = { ...process.env };
    vi.clearAllMocks();
  });

  afterEach(() => {
    // Restore original environment
    process.env = originalEnv;
    // Clear module cache to ensure fresh imports
    vi.resetModules();
  });

  describe('isProduction', () => {
    it('should be true when app is packaged', async () => {
      const { app } = await import('electron');
      (app as { isPackaged: boolean }).isPackaged = true;

      // Re-import to get updated value
      const { isProduction } = await import('@/main/utils/env');
      expect(isProduction).toBe(true);
    });

    it('should be false when app is not packaged', async () => {
      const { app } = await import('electron');
      (app as { isPackaged: boolean }).isPackaged = false;

      // Re-import to get updated value
      const { isProduction } = await import('@/main/utils/env');
      expect(isProduction).toBe(false);
    });
  });

  describe('isDev', () => {
    it('should be false when app is packaged', async () => {
      const { app } = await import('electron');
      (app as { isPackaged: boolean }).isPackaged = true;

      // Re-import to get updated value
      const { isDev } = await import('@/main/utils/env');
      expect(isDev).toBe(false);
    });

    it('should be true when app is not packaged', async () => {
      const { app } = await import('electron');
      (app as { isPackaged: boolean }).isPackaged = false;

      // Re-import to get updated value
      const { isDev } = await import('@/main/utils/env');
      expect(isDev).toBe(true);
    });

    it('should be inverse of isProduction', async () => {
      const { isProduction, isDev } = await import('@/main/utils/env');
      expect(isDev).toBe(!isProduction);
    });
  });

  describe('devServerUrl', () => {
    it('should return the VITE_DEV_SERVER_URL when set', async () => {
      process.env.VITE_DEV_SERVER_URL = 'http://localhost:5173';

      // Re-import to get updated value
      const { devServerUrl } = await import('@/main/utils/env');
      expect(devServerUrl).toBe('http://localhost:5173');
    });

    it('should return undefined when VITE_DEV_SERVER_URL is not set', async () => {
      delete process.env.VITE_DEV_SERVER_URL;

      // Re-import to get updated value
      const { devServerUrl } = await import('@/main/utils/env');
      expect(devServerUrl).toBeUndefined();
    });

    it('should handle different dev server URLs', async () => {
      const testUrls = [
        'http://localhost:3000',
        'http://127.0.0.1:5173',
        'http://0.0.0.0:8080',
      ];

      for (const url of testUrls) {
        process.env.VITE_DEV_SERVER_URL = url;
        vi.resetModules();

        const { devServerUrl } = await import('@/main/utils/env');
        expect(devServerUrl).toBe(url);
      }
    });
  });

  describe('getAppVersion', () => {
    it('returns app.getVersion() by default', async () => {
      const { app } = await import('electron');
      const mockApp = app as unknown as {
        isPackaged: boolean;
        getVersion: () => string;
      };
      mockApp.isPackaged = true;
      vi.resetModules();
      const env = await import('@/main/utils/env');
      const version = env.getAppVersion();
      expect(typeof version).toBe('string');
    });

    it('returns CAPTY_DEV_APP_VERSION when set in dev', async () => {
      const { app } = await import('electron');
      const mockApp = app as unknown as {
        isPackaged: boolean;
        getVersion: () => string;
      };
      mockApp.isPackaged = false;
      process.env.CAPTY_DEV_APP_VERSION = '99.99.99';
      vi.resetModules();
      const env = await import('@/main/utils/env');
      expect(env.getAppVersion()).toBe('99.99.99');
      delete process.env.CAPTY_DEV_APP_VERSION;
    });
  });

  describe('Environment consistency', () => {
    it('should ensure isDev and isProduction are mutually exclusive', async () => {
      const { app } = await import('electron');

      // Test packaged mode
      (app as { isPackaged: boolean }).isPackaged = true;
      vi.resetModules();
      let env = await import('@/main/utils/env');
      expect(env.isProduction && !env.isDev).toBe(true);

      // Test development mode
      (app as { isPackaged: boolean }).isPackaged = false;
      vi.resetModules();
      env = await import('@/main/utils/env');
      expect(!env.isProduction && env.isDev).toBe(true);
    });
  });
});
