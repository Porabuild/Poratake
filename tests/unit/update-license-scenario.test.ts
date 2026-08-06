import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

const mockApp = {
  getVersion: vi.fn(() => '0.32.1'),
};

const mockIpcMainHandlers = new Map<string, (...args: unknown[]) => unknown>();
const mockIpcMain = {
  handle: vi.fn((channel: string, handler: (...args: unknown[]) => unknown) => {
    mockIpcMainHandlers.set(channel, handler);
  }),
  on: vi.fn(),
};

vi.mock('electron', () => ({
  app: mockApp,
  ipcMain: mockIpcMain,
}));

vi.mock('@/main/utils/env', () => ({
  isDev: false,
  isProduction: true,
  getAppVersion: () => mockApp.getVersion(),
}));

type AutoUpdaterEventHandler = (...args: unknown[]) => void;
const autoUpdaterEventHandlers = new Map<string, AutoUpdaterEventHandler>();

const mockAutoUpdater = {
  setFeedURL: vi.fn(),
  checkForUpdates: vi.fn(() => Promise.resolve()),
  downloadUpdate: vi.fn(),
  quitAndInstall: vi.fn(),
  autoDownload: false,
  autoInstallOnAppQuit: false,
  on: vi.fn((event: string, handler: AutoUpdaterEventHandler) => {
    autoUpdaterEventHandlers.set(event, handler);
  }),
};

vi.mock('electron-updater', () => ({
  autoUpdater: mockAutoUpdater,
}));

const mockRebuildTrayMenu = vi.fn();
vi.mock('@/main/menu/index', () => ({
  rebuildTrayMenu: mockRebuildTrayMenu,
}));

vi.mock('@/main/update/config', () => ({
  API_URL: 'https://test.capty.app',
}));

const mockBroadcastUpdateEvent = vi.fn();
vi.mock('@/main/update/broadcast', () => ({
  broadcastUpdateEvent: mockBroadcastUpdateEvent,
}));

vi.mock('@/main/utils/paths', () => ({
  getConfigDir: vi.fn(() => '/mock/config'),
  getLicenseFilePath: vi.fn(() => '/mock/config/license.json'),
  getTrialFilePath: vi.fn(() => '/mock/config/trial.json'),
}));

vi.mock('fs', () => ({
  existsSync: vi.fn(() => true),
}));

vi.mock('fs/promises', () => ({
  readFile: vi.fn(() => Promise.resolve('0.32.1')),
  writeFile: vi.fn(() => Promise.resolve()),
  mkdir: vi.fn(() => Promise.resolve()),
}));

vi.mock('@/main/capture', () => ({
  resetScreenCaptureCache: vi.fn(),
}));

const mockGetLicenseStatus = vi.fn(() => 'not_activated');
const mockGetTrialStatus = vi.fn(() => 'unknown');

vi.mock('@/main/license/cache', () => ({
  getLicenseStatus: mockGetLicenseStatus,
  getTrialStatus: mockGetTrialStatus,
}));

describe('Update + license scenario', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    autoUpdaterEventHandlers.clear();
    mockIpcMainHandlers.clear();
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.resetModules();
  });

  it('downloads 1.0.0 and is not pro without license', async () => {
    mockApp.getVersion.mockReturnValue('0.32.1');

    const { init, getUpdateState } = await import('@/main/update/index');
    init();

    const availableHandler = autoUpdaterEventHandlers.get('update-available');
    availableHandler?.({
      version: '1.0.0',
      releaseNotes: null,
    });

    expect(mockAutoUpdater.downloadUpdate).toHaveBeenCalled();

    const downloadedHandler = autoUpdaterEventHandlers.get('update-downloaded');
    downloadedHandler?.({ version: '1.0.0' });

    const state = getUpdateState();
    expect(state.status).toBe('ready');
    expect(state.latestVersion).toBe('1.0.0');

    const installHandler = mockIpcMainHandlers.get('update:install');
    installHandler?.();
    expect(mockAutoUpdater.quitAndInstall).toHaveBeenCalledWith(false, true);

    vi.resetModules();
    mockApp.getVersion.mockReturnValue('1.0.0');
    mockGetLicenseStatus.mockReturnValue('not_activated');

    const { isPro } = await import('@/main/license/validation');
    expect(isPro()).toBe(false);
  });
});
