import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Store IPC handlers for testing
const ipcHandlers: Record<string, (...args: unknown[]) => unknown> = {};

const mockIpcMain = {
  handle: vi.fn((channel: string, handler: (...args: unknown[]) => unknown) => {
    ipcHandlers[channel] = handler;
  }),
};

vi.mock('electron', () => ({
  app: { isPackaged: false },
  ipcMain: mockIpcMain,
  BrowserWindow: {
    getAllWindows: vi.fn(() => []),
  },
}));

vi.mock('@/main/utils/env', () => ({
  isDev: false,
  isProduction: true,
}));

const mockInitSSLSettings = vi.fn();

vi.mock('@/main/license/config', () => ({
  initSSLSettings: mockInitSSLSettings,
}));

const mockCacheFunctions = {
  getCachedLicense: vi.fn(),
  getLicenseStatus: vi.fn(() => 'valid'),
  loadCachedLicense: vi.fn(),
  setCachedLicense: vi.fn(),
  setLicenseStatus: vi.fn(),
};

vi.mock('@/main/license/cache', () => mockCacheFunctions);

vi.mock('@/main/license/device', () => ({
  generateDeviceFingerprint: vi.fn(() => 'mock-fingerprint'),
}));

const mockValidationFunctions = {
  isPro: vi.fn(() => false),
  isFirstTimeActivation: vi.fn(() => false),
};

vi.mock('@/main/license/validation', () => mockValidationFunctions);

const mockApiFunctions = {
  activateLicense: vi.fn(),
  validateLicense: vi.fn(),
  deactivateLicense: vi.fn(),
  getCheckoutUrl: vi.fn(() => 'https://capty.app'),
};

vi.mock('@/main/license/api', () => mockApiFunctions);

describe('License Init', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.keys(ipcHandlers).forEach(key => delete ipcHandlers[key]);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('init', () => {
    it('should call initSSLSettings on startup', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);

      const { init } = await import('@/main/license/index');
      await init();

      expect(mockInitSSLSettings).toHaveBeenCalled();
    });

    it('should register all IPC handlers', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);

      const { init } = await import('@/main/license/index');
      await init();

      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'license:getStatus',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'license:activate',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'license:validate',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'license:deactivate',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'license:isPro',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'license:getCheckoutUrl',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'license:isFirstTimeActivation',
        expect.any(Function)
      );
    });

    it('should load cached license on startup', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);

      const { init } = await import('@/main/license/index');
      await init();

      expect(mockCacheFunctions.loadCachedLicense).toHaveBeenCalled();
      expect(mockCacheFunctions.setCachedLicense).toHaveBeenCalledWith(null);
    });

    it('should set status to not_activated when no cached license', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);
      mockCacheFunctions.getCachedLicense.mockReturnValue(null);

      const { init } = await import('@/main/license/index');
      await init();

      expect(mockCacheFunctions.setLicenseStatus).toHaveBeenCalledWith(
        'not_activated'
      );
      expect(mockApiFunctions.validateLicense).not.toHaveBeenCalled();
    });

    it('should validate license when cached license exists', async () => {
      const cachedLicense = {
        licenseKey: 'TEST-KEY',
        email: 'test@example.com',
        expiresAt: '2025-12-31',
        maxVersion: '2.0.0',
        isLifetime: false,
      };
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(cachedLicense);
      mockCacheFunctions.getCachedLicense.mockReturnValue(cachedLicense);
      mockApiFunctions.validateLicense.mockResolvedValue({ valid: true });

      const { init } = await import('@/main/license/index');
      await init();

      expect(mockCacheFunctions.setCachedLicense).toHaveBeenCalledWith(
        cachedLicense
      );
      expect(mockCacheFunctions.setLicenseStatus).not.toHaveBeenCalledWith(
        'not_activated'
      );
      expect(mockApiFunctions.validateLicense).toHaveBeenCalled();
    });
  });

  describe('IPC handler: license:getStatus', () => {
    it('should return status and null info when no cached license', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);
      mockCacheFunctions.getCachedLicense.mockReturnValue(null);
      mockCacheFunctions.getLicenseStatus.mockReturnValue('not_activated');

      const { init } = await import('@/main/license/index');
      await init();

      const result = ipcHandlers['license:getStatus']();

      expect(result).toEqual({
        status: 'not_activated',
        info: null,
      });
    });

    it('should return status and license info when cached license exists', async () => {
      const cachedLicense = {
        licenseKey: 'TEST-KEY',
        email: 'test@example.com',
        expiresAt: '2025-12-31',
        maxVersion: '2.0.0',
        isLifetime: true,
      };
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(cachedLicense);
      mockCacheFunctions.getCachedLicense.mockReturnValue(cachedLicense);
      mockCacheFunctions.getLicenseStatus.mockReturnValue('valid');
      mockApiFunctions.validateLicense.mockResolvedValue({ valid: true });

      const { init } = await import('@/main/license/index');
      await init();

      const result = ipcHandlers['license:getStatus']();

      expect(result).toEqual({
        status: 'valid',
        info: {
          email: 'test@example.com',
          expiresAt: '2025-12-31',
          maxVersion: '2.0.0',
          isLifetime: true,
        },
      });
    });
  });

  describe('IPC handler: license:activate', () => {
    it('should call activateLicense with provided credentials', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);
      const mockResponse = { valid: true };
      mockApiFunctions.activateLicense.mockResolvedValue(mockResponse);

      const { init } = await import('@/main/license/index');
      await init();

      const result = await ipcHandlers['license:activate'](
        {},
        'user@test.com',
        'LICENSE-KEY-123'
      );

      expect(mockApiFunctions.activateLicense).toHaveBeenCalledWith(
        'user@test.com',
        'LICENSE-KEY-123'
      );
      expect(result).toEqual(mockResponse);
    });
  });

  describe('IPC handler: license:validate', () => {
    it('should call validateLicense', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);
      const mockResponse = { valid: true };
      mockApiFunctions.validateLicense.mockResolvedValue(mockResponse);

      const { init } = await import('@/main/license/index');
      await init();

      const result = await ipcHandlers['license:validate']();

      expect(mockApiFunctions.validateLicense).toHaveBeenCalled();
      expect(result).toEqual(mockResponse);
    });
  });

  describe('IPC handler: license:deactivate', () => {
    it('should call deactivateLicense', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);
      mockApiFunctions.deactivateLicense.mockResolvedValue(true);

      const { init } = await import('@/main/license/index');
      await init();

      const result = await ipcHandlers['license:deactivate']();

      expect(mockApiFunctions.deactivateLicense).toHaveBeenCalled();
      expect(result).toBe(true);
    });
  });

  describe('IPC handler: license:isPro', () => {
    it('should call isPro', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);
      mockValidationFunctions.isPro.mockReturnValue(true);

      const { init } = await import('@/main/license/index');
      await init();

      const result = ipcHandlers['license:isPro']();

      expect(mockValidationFunctions.isPro).toHaveBeenCalled();
      expect(result).toBe(true);
    });
  });

  describe('IPC handler: license:getCheckoutUrl', () => {
    it('should call getCheckoutUrl', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);
      mockApiFunctions.getCheckoutUrl.mockReturnValue('https://checkout.url');

      const { init } = await import('@/main/license/index');
      await init();

      const result = ipcHandlers['license:getCheckoutUrl']();

      expect(mockApiFunctions.getCheckoutUrl).toHaveBeenCalled();
      expect(result).toBe('https://checkout.url');
    });
  });

  describe('IPC handler: license:isFirstTimeActivation', () => {
    it('should call isFirstTimeActivation', async () => {
      mockCacheFunctions.loadCachedLicense.mockResolvedValue(null);
      mockValidationFunctions.isFirstTimeActivation.mockReturnValue(true);

      const { init } = await import('@/main/license/index');
      await init();

      const result = ipcHandlers['license:isFirstTimeActivation']();

      expect(mockValidationFunctions.isFirstTimeActivation).toHaveBeenCalled();
      expect(result).toBe(true);
    });
  });

  describe('re-exports', () => {
    it('should re-export activateLicense from api', async () => {
      const module = await import('@/main/license/index');
      expect(module.activateLicense).toBeDefined();
    });

    it('should re-export validateLicense from api', async () => {
      const module = await import('@/main/license/index');
      expect(module.validateLicense).toBeDefined();
    });

    it('should re-export deactivateLicense from api', async () => {
      const module = await import('@/main/license/index');
      expect(module.deactivateLicense).toBeDefined();
    });

    it('should re-export getLicenseStatus from cache', async () => {
      const module = await import('@/main/license/index');
      expect(module.getLicenseStatus).toBeDefined();
    });

    it('should re-export getLicenseInfo (getCachedLicense) from cache', async () => {
      const module = await import('@/main/license/index');
      expect(module.getLicenseInfo).toBeDefined();
    });

    it('should re-export isPro from validation', async () => {
      const module = await import('@/main/license/index');
      expect(module.isPro).toBeDefined();
    });

    it('should re-export isFirstTimeActivation from validation', async () => {
      const module = await import('@/main/license/index');
      expect(module.isFirstTimeActivation).toBeDefined();
    });

    it('should re-export generateDeviceFingerprint from device', async () => {
      const module = await import('@/main/license/index');
      expect(module.generateDeviceFingerprint).toBeDefined();
    });
  });
});
