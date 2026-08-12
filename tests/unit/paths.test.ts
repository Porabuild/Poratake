import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import path from 'path';

// Create mock app object that can be mutated
const mockApp = {
  getPath: vi.fn((name: string) => {
    if (name === 'home') return '/mock/home';
    return `/mock/${name}`;
  }),
  getAppPath: vi.fn(() => '/mock/app'),
  isPackaged: false,
};

// Mock fs for getNativeBinaryPath tests
const mockExistsSync = vi.fn((_path: string) => true);

vi.mock('fs', () => ({
  default: { existsSync: mockExistsSync },
  existsSync: mockExistsSync,
}));

// Mock electron module
vi.mock('electron', () => ({
  app: mockApp,
}));

describe('Path Utilities', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset to default values
    mockApp.isPackaged = false;
    mockApp.getPath = vi.fn((name: string) => {
      if (name === 'home') return '/mock/home';
      return `/mock/${name}`;
    });
    mockApp.getAppPath = vi.fn(() => '/mock/app');
    mockExistsSync.mockReturnValue(true);
    // Clear module cache to ensure fresh imports
    vi.resetModules();
  });

  afterEach(() => {
    vi.resetModules();
  });

  describe('getConfigDir', () => {
    it('should return development config directory when in dev mode', async () => {
      mockApp.isPackaged = false;

      const { getConfigDir } = await import('@/main/utils/paths');
      const configDir = getConfigDir();
      expect(configDir).toBe(
        path.join('/mock/home', '.config', 'poratake-dev')
      );
    });

    it('should return production config directory when packaged', async () => {
      mockApp.isPackaged = true;

      const { getConfigDir } = await import('@/main/utils/paths');
      const configDir = getConfigDir();
      expect(configDir).toBe(path.join('/mock/home', '.config', 'poratake'));
    });

    it('should use home directory from Electron app', async () => {
      mockApp.getPath = vi.fn((name: string) => {
        if (name === 'home') return '/custom/home/path';
        return `/custom/${name}`;
      });

      const { getConfigDir } = await import('@/main/utils/paths');
      const configDir = getConfigDir();
      expect(configDir).toContain(path.join('/custom/home/path'));
      expect(mockApp.getPath).toHaveBeenCalledWith('home');
    });
  });

  describe('getConfigFilePath', () => {
    it('should return correct config file path in dev mode', async () => {
      mockApp.isPackaged = false;

      const { getConfigFilePath } = await import('@/main/utils/paths');
      const configPath = getConfigFilePath();
      expect(configPath).toBe(
        path.join('/mock/home', '.config', 'poratake-dev', 'config.json')
      );
    });

    it('should return correct config file path in production', async () => {
      mockApp.isPackaged = true;

      const { getConfigFilePath } = await import('@/main/utils/paths');
      const configPath = getConfigFilePath();
      expect(configPath).toBe(
        path.join('/mock/home', '.config', 'poratake', 'config.json')
      );
    });

    it('should always end with config.json', async () => {
      const { getConfigFilePath } = await import('@/main/utils/paths');
      const configPath = getConfigFilePath();
      expect(configPath).toMatch(/config\.json$/);
    });
  });

  describe('getLicenseFilePath', () => {
    it('should return correct license file path in dev mode', async () => {
      mockApp.isPackaged = false;

      const { getLicenseFilePath } = await import('@/main/utils/paths');
      const licensePath = getLicenseFilePath();
      expect(licensePath).toBe(
        path.join('/mock/home', '.config', 'poratake-dev', 'license.json')
      );
    });

    it('should return correct license file path in production', async () => {
      mockApp.isPackaged = true;

      const { getLicenseFilePath } = await import('@/main/utils/paths');
      const licensePath = getLicenseFilePath();
      expect(licensePath).toBe(
        path.join('/mock/home', '.config', 'poratake', 'license.json')
      );
    });

    it('should always end with license.json', async () => {
      const { getLicenseFilePath } = await import('@/main/utils/paths');
      const licensePath = getLicenseFilePath();
      expect(licensePath).toMatch(/license\.json$/);
    });
  });

  describe('getHistoryFilePath', () => {
    it('should return correct history file path in dev mode', async () => {
      mockApp.isPackaged = false;

      const { getHistoryFilePath } = await import('@/main/utils/paths');
      const historyPath = getHistoryFilePath();
      expect(historyPath).toBe(
        path.join('/mock/home', '.config', 'poratake-dev', 'history.json')
      );
    });

    it('should return correct history file path in production', async () => {
      mockApp.isPackaged = true;

      const { getHistoryFilePath } = await import('@/main/utils/paths');
      const historyPath = getHistoryFilePath();
      expect(historyPath).toBe(
        path.join('/mock/home', '.config', 'poratake', 'history.json')
      );
    });

    it('should always end with history.json', async () => {
      const { getHistoryFilePath } = await import('@/main/utils/paths');
      const historyPath = getHistoryFilePath();
      expect(historyPath).toMatch(/history\.json$/);
    });
  });

  describe('Path consistency', () => {
    it('should ensure all config-related paths use the same config directory', async () => {
      const {
        getConfigDir,
        getConfigFilePath,
        getLicenseFilePath,
        getHistoryFilePath,
      } = await import('@/main/utils/paths');

      const configDir = getConfigDir();
      const configPath = getConfigFilePath();
      const licensePath = getLicenseFilePath();
      const historyPath = getHistoryFilePath();

      expect(configPath).toContain(configDir);
      expect(licensePath).toContain(configDir);
      expect(historyPath).toContain(configDir);
    });

    it('should separate dev and production configs', async () => {
      mockApp.isPackaged = false;
      const { getConfigDir: getConfigDirDev } =
        await import('@/main/utils/paths');
      const devConfigDir = getConfigDirDev();

      vi.resetModules();
      mockApp.isPackaged = true;
      const { getConfigDir: getConfigDirProd } =
        await import('@/main/utils/paths');
      const prodConfigDir = getConfigDirProd();

      expect(devConfigDir).not.toBe(prodConfigDir);
      expect(devConfigDir).toContain('poratake-dev');
      expect(prodConfigDir).toContain('poratake');
      expect(prodConfigDir).not.toContain('poratake-dev');
    });
  });

  describe('getNativeBinaryPath', () => {
    it('should return dev path when it exists', async () => {
      const devPath = path.join(
        '/mock/app',
        'src/main/binaries/test-binary',
        'test-binary'
      );
      mockExistsSync.mockImplementation((p: string) => p === devPath);

      const { getNativeBinaryPath } = await import('@/main/utils/paths');
      const binaryPath = getNativeBinaryPath('test-binary');

      expect(binaryPath).toBe(devPath);
    });

    it('should return prod path when dev path does not exist', async () => {
      const originalResourcesPath = process.resourcesPath;
      Object.defineProperty(process, 'resourcesPath', {
        value: '/mock/resources',
        writable: true,
        configurable: true,
      });

      const prodPath = path.join(
        '/mock/resources',
        'binaries/test-binary',
        'test-binary'
      );
      mockExistsSync.mockImplementation((p: string) => p === prodPath);

      const { getNativeBinaryPath } = await import('@/main/utils/paths');
      const binaryPath = getNativeBinaryPath('test-binary');

      expect(binaryPath).toBe(prodPath);

      Object.defineProperty(process, 'resourcesPath', {
        value: originalResourcesPath,
        writable: true,
        configurable: true,
      });
    });

    it('should fallback to dev path when neither exists', async () => {
      mockExistsSync.mockReturnValue(false);

      const { getNativeBinaryPath } = await import('@/main/utils/paths');
      const binaryPath = getNativeBinaryPath('missing-binary');

      expect(binaryPath).toBe(
        path.join(
          '/mock/app',
          'src/main/binaries/missing-binary',
          'missing-binary'
        )
      );
    });

    it('should work with different binary names', async () => {
      mockExistsSync.mockReturnValue(true);

      const { getNativeBinaryPath } = await import('@/main/utils/paths');

      expect(getNativeBinaryPath('ffmpeg')).toBe(
        path.join('/mock/app', 'src/main/binaries/ffmpeg', 'ffmpeg')
      );
      expect(getNativeBinaryPath('capty-daemon')).toBe(
        path.join('/mock/app', 'src/main/daemon', 'capty-daemon')
      );
    });
  });
});
