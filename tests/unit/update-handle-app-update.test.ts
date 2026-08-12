import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'path';

const mockExistsSync = vi.fn();
const mockReadFile = vi.fn();
const mockWriteFile = vi.fn();
const mockMkdir = vi.fn();
const mockResetScreenCaptureCache = vi.fn();
const mockGetAppVersion = vi.fn(() => '1.2.3');
const mockGetConfigDir = vi.fn(() => '/cfg');

vi.mock('electron', () => ({
  ipcMain: { handle: vi.fn() },
  app: { isPackaged: false, getVersion: () => '1.2.3' },
}));

vi.mock('electron-updater', () => ({
  autoUpdater: {
    setFeedURL: vi.fn(),
    autoDownload: false,
    autoInstallOnAppQuit: true,
    on: vi.fn(),
    checkForUpdates: vi.fn(),
    quitAndInstall: vi.fn(),
    downloadUpdate: vi.fn(),
  },
}));

vi.mock('fs', () => ({
  default: { existsSync: (...a: unknown[]) => mockExistsSync(...a) },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
}));

vi.mock('fs/promises', () => ({
  default: {
    readFile: (...a: unknown[]) => mockReadFile(...a),
    writeFile: (...a: unknown[]) => mockWriteFile(...a),
    mkdir: (...a: unknown[]) => mockMkdir(...a),
  },
  readFile: (...a: unknown[]) => mockReadFile(...a),
  writeFile: (...a: unknown[]) => mockWriteFile(...a),
  mkdir: (...a: unknown[]) => mockMkdir(...a),
}));

vi.mock('@/main/utils/env.ts', () => ({
  isDev: false,
  isProduction: true,
  getAppVersion: () => mockGetAppVersion(),
  devServerUrl: undefined,
}));

vi.mock('@/main/utils/env', () => ({
  isDev: false,
  isProduction: true,
  getAppVersion: () => mockGetAppVersion(),
  devServerUrl: undefined,
}));

vi.mock('@/main/utils/paths.ts', () => ({
  getConfigDir: () => mockGetConfigDir(),
  getConfigFilePath: () => '/cfg/config.json',
  getLicenseFilePath: () => '/cfg/license.json',
  getTrialFilePath: () => '/cfg/trial.json',
  getHistoryFilePath: () => '/cfg/history.json',
  getNativeBinaryPath: (n: string) => `/bin/${n}`,
  getPublicAssetPath: (r: string) => `/assets/${r}`,
  ensureDirectoryExists: (p: string) => p,
  isValidDirectory: () => true,
}));

vi.mock('@/main/utils/paths', () => ({
  getConfigDir: () => mockGetConfigDir(),
  getConfigFilePath: () => '/cfg/config.json',
  getLicenseFilePath: () => '/cfg/license.json',
  getTrialFilePath: () => '/cfg/trial.json',
  getHistoryFilePath: () => '/cfg/history.json',
  getNativeBinaryPath: (n: string) => `/bin/${n}`,
  getPublicAssetPath: (r: string) => `/assets/${r}`,
  ensureDirectoryExists: (p: string) => p,
  isValidDirectory: () => true,
}));

vi.mock('@/main/menu', () => ({
  rebuildTrayMenu: vi.fn(),
}));

vi.mock('@/main/capture', () => ({
  resetScreenCaptureCache: () => mockResetScreenCaptureCache(),
}));

describe('handleAppUpdate', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it('creates config dir if missing', async () => {
    mockExistsSync.mockReturnValue(false);
    const { handleAppUpdate } = await import('@/main/update');
    await handleAppUpdate();
    expect(mockMkdir).toHaveBeenCalledWith('/cfg', { recursive: true });
  });

  it('resets cache when version differs from stored', async () => {
    const versionFile = path.join('/cfg', '.last-version');
    mockExistsSync.mockImplementation(
      (p: string) => p === '/cfg' || p === versionFile
    );
    mockReadFile.mockResolvedValue('1.0.0');
    const { handleAppUpdate } = await import('@/main/update');
    await handleAppUpdate();
    expect(mockResetScreenCaptureCache).toHaveBeenCalled();
    expect(mockWriteFile).toHaveBeenCalledWith(versionFile, '1.2.3', 'utf-8');
  });

  it('does not reset when version matches', async () => {
    mockExistsSync.mockReturnValue(true);
    mockReadFile.mockResolvedValue('1.2.3');
    const { handleAppUpdate } = await import('@/main/update');
    await handleAppUpdate();
    expect(mockResetScreenCaptureCache).not.toHaveBeenCalled();
  });

  it('treats missing file as first run', async () => {
    mockExistsSync.mockImplementation((p: string) => p === '/cfg');
    const { handleAppUpdate } = await import('@/main/update');
    await handleAppUpdate();
    expect(mockResetScreenCaptureCache).toHaveBeenCalled();
  });

  it('resets cache when error occurs', async () => {
    mockExistsSync.mockImplementation(() => {
      throw new Error('fs error');
    });
    const { handleAppUpdate } = await import('@/main/update');
    await handleAppUpdate();
    expect(mockResetScreenCaptureCache).toHaveBeenCalled();
  });
});
