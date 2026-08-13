import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

import type { UpdateState } from '@/types/update';

// Mock electron
const mockApp = {
  getVersion: vi.fn(() => '1.0.0'),
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

// Mock electron-updater
type AutoUpdaterEventHandler = (...args: unknown[]) => void;
const autoUpdaterEventHandlers = new Map<string, AutoUpdaterEventHandler>();

const mockAutoUpdater = {
  setFeedURL: vi.fn(),
  checkForUpdates: vi.fn(() => Promise.resolve()),
  downloadUpdate: vi.fn(() => Promise.resolve([])),
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

const mockPlatform = { isMac: true };
vi.mock('@/main/utils/platform', () => ({
  get isMac() {
    return mockPlatform.isMac;
  },
}));

// Mock menu
const mockRebuildTrayMenu = vi.fn();
vi.mock('@/main/menu/index', () => ({
  rebuildTrayMenu: mockRebuildTrayMenu,
}));

vi.mock('@/main/update/config', () => ({
  UPDATE_OWNER: 'Porabuild',
  UPDATE_REPOSITORY: 'Poratake',
}));

// Mock broadcast
const mockBroadcastUpdateEvent = vi.fn();
vi.mock('@/main/update/broadcast', () => ({
  broadcastUpdateEvent: mockBroadcastUpdateEvent,
}));

// Mock paths
vi.mock('@/main/utils/paths', () => ({
  getConfigDir: vi.fn(() => '/mock/config'),
}));

// Mock fs
vi.mock('fs', () => ({
  existsSync: vi.fn(() => true),
}));

// Mock fs/promises
vi.mock('fs/promises', () => ({
  readFile: vi.fn(() => Promise.resolve('1.0.0')),
  writeFile: vi.fn(() => Promise.resolve()),
  mkdir: vi.fn(() => Promise.resolve()),
}));

// Mock capture
vi.mock('@/main/capture', () => ({
  resetScreenCaptureCache: vi.fn(),
}));

describe('Update System', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    autoUpdaterEventHandlers.clear();
    mockIpcMainHandlers.clear();
    mockPlatform.isMac = true;
    mockAutoUpdater.checkForUpdates.mockResolvedValue(undefined);
    mockAutoUpdater.downloadUpdate.mockResolvedValue([]);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.resetModules();
  });

  describe('getUpdateState', () => {
    it('should return a copy of the update state', async () => {
      const { getUpdateState } = await import('@/main/update/index');

      const state = getUpdateState();

      expect(state).toHaveProperty('status');
      expect(state).toHaveProperty('currentVersion');
      expect(state).toHaveProperty('latestVersion');
      expect(state).toHaveProperty('releaseNotes');
      expect(state).toHaveProperty('downloadProgress');
      expect(state).toHaveProperty('error');
    });

    it('should return default idle status initially', async () => {
      const { getUpdateState } = await import('@/main/update/index');

      const state = getUpdateState();

      expect(state.status).toBe('idle');
      expect(state.downloadProgress).toBe(0);
      expect(state.error).toBeNull();
    });
  });

  describe('init', () => {
    it('should set current version from app', async () => {
      mockApp.getVersion.mockReturnValue('2.5.0');

      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const state = getUpdateState();
      expect(state.currentVersion).toBe('2.5.0');
    });

    it('should configure auto updater with correct feed URL', async () => {
      const { init } = await import('@/main/update/index');
      init();

      expect(mockAutoUpdater.setFeedURL).toHaveBeenCalledWith({
        provider: 'github',
        owner: 'Porabuild',
        repo: 'Poratake',
      });
    });

    it('should disable auto download', async () => {
      const { init } = await import('@/main/update/index');
      init();

      expect(mockAutoUpdater.autoDownload).toBe(false);
    });

    it('should enable auto install on quit', async () => {
      const { init } = await import('@/main/update/index');
      init();

      expect(mockAutoUpdater.autoInstallOnAppQuit).toBe(true);
    });

    it('should register IPC handlers', async () => {
      const { init } = await import('@/main/update/index');
      init();

      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'update:getState',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'update:check',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'update:install',
        expect.any(Function)
      );
    });

    it('should check for updates after initial delay', async () => {
      const { init } = await import('@/main/update/index');
      init();

      expect(mockAutoUpdater.checkForUpdates).not.toHaveBeenCalled();

      vi.advanceTimersByTime(3000); // INITIAL_CHECK_DELAY

      expect(mockAutoUpdater.checkForUpdates).toHaveBeenCalled();
    });

    it('should start periodic update checks', async () => {
      const { init } = await import('@/main/update/index');
      init();

      // Initial delay
      vi.advanceTimersByTime(3000);
      expect(mockAutoUpdater.checkForUpdates).toHaveBeenCalledTimes(1);

      // First periodic check (30 minutes)
      vi.advanceTimersByTime(30 * 60 * 1000);
      expect(mockAutoUpdater.checkForUpdates).toHaveBeenCalledTimes(2);

      // Second periodic check
      vi.advanceTimersByTime(30 * 60 * 1000);
      expect(mockAutoUpdater.checkForUpdates).toHaveBeenCalledTimes(3);
    });

    it('should report unsupported without configuring update checks on Windows', async () => {
      mockPlatform.isMac = false;
      const { init, getUpdateState } = await import('@/main/update/index');

      init();
      vi.advanceTimersByTime(30 * 60 * 1000);

      expect(getUpdateState().status).toBe('unsupported');
      expect(mockAutoUpdater.setFeedURL).not.toHaveBeenCalled();
      expect(mockAutoUpdater.checkForUpdates).not.toHaveBeenCalled();
    });
  });

  describe('Auto updater event handlers', () => {
    it('should set status to checking on checking-for-update event', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('checking-for-update');
      handler?.();

      expect(getUpdateState().status).toBe('checking');
      expect(mockBroadcastUpdateEvent).toHaveBeenCalledWith(
        'update:status-changed',
        expect.objectContaining({ status: 'checking' })
      );
    });

    it('should handle update-available event with string release notes', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('update-available');
      handler?.({
        version: '2.0.0',
        releaseNotes: 'New features and bug fixes',
      });

      const state = getUpdateState();
      expect(state.status).toBe('available');
      expect(state.latestVersion).toBe('2.0.0');
      expect(state.releaseNotes).toBe('New features and bug fixes');
      expect(mockAutoUpdater.downloadUpdate).toHaveBeenCalled();
    });

    it('should handle update-available event with array release notes', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('update-available');
      handler?.({
        version: '2.0.0',
        releaseNotes: [{ note: 'First note' }, { note: 'Second note' }],
      });

      const state = getUpdateState();
      expect(state.releaseNotes).toBe('First note');
    });

    it('should handle update-available event without release notes', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('update-available');
      handler?.({
        version: '2.0.0',
        releaseNotes: undefined,
      });

      const state = getUpdateState();
      expect(state.releaseNotes).toBeNull();
    });

    it('should rebuild menu menu when update becomes available', async () => {
      const { init } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('update-available');
      handler?.({ version: '2.0.0', releaseNotes: null });

      expect(mockRebuildTrayMenu).toHaveBeenCalled();
    });

    it('should handle update-not-available event', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('update-not-available');
      handler?.({ version: '1.0.0' });

      const state = getUpdateState();
      expect(state.status).toBe('up_to_date');
      expect(state.latestVersion).toBe('1.0.0');
    });

    it('should handle download-progress event', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('download-progress');
      handler?.({ percent: 45.7 });

      const state = getUpdateState();
      expect(state.status).toBe('downloading');
      expect(state.downloadProgress).toBe(46); // Rounded
      expect(mockBroadcastUpdateEvent).toHaveBeenCalledWith(
        'update:download-progress',
        45.7
      );
    });

    it('should rebuild menu menu every 10% progress', async () => {
      const { init } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('download-progress');
      mockRebuildTrayMenu.mockClear();

      handler?.({ percent: 5 });
      expect(mockRebuildTrayMenu).not.toHaveBeenCalled();

      handler?.({ percent: 10 });
      expect(mockRebuildTrayMenu).toHaveBeenCalledTimes(1);

      handler?.({ percent: 15 });
      expect(mockRebuildTrayMenu).toHaveBeenCalledTimes(1);

      handler?.({ percent: 20 });
      expect(mockRebuildTrayMenu).toHaveBeenCalledTimes(2);
    });

    it('should handle update-downloaded event', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('update-downloaded');
      handler?.({ version: '2.0.0' });

      const state = getUpdateState();
      expect(state.status).toBe('ready');
      expect(state.latestVersion).toBe('2.0.0');
      expect(state.downloadProgress).toBe(100);
    });

    it('should rebuild menu menu when update is ready', async () => {
      const { init } = await import('@/main/update/index');
      init();

      mockRebuildTrayMenu.mockClear();
      const handler = autoUpdaterEventHandlers.get('update-downloaded');
      handler?.({ version: '2.0.0' });

      expect(mockRebuildTrayMenu).toHaveBeenCalled();
    });

    it('should handle error event', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('error');
      handler?.(new Error('Download failed'));

      const state = getUpdateState();
      expect(state.status).toBe('error');
      expect(state.error).toBe('Download failed');
    });

    it('should report a missing channel file', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('error');
      const error = new Error('Not Found 404');
      (error as Error & { code?: string }).code =
        'ERR_UPDATER_CHANNEL_FILE_NOT_FOUND';
      handler?.(error);

      const state = getUpdateState();
      expect(state.status).toBe('error');
      expect(state.error).toBe('Not Found 404');
    });

    it('should report asset 404 errors', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = autoUpdaterEventHandlers.get('error');
      handler?.(new Error('HTTP error 404 not found'));

      const state = getUpdateState();
      expect(state.status).toBe('error');
      expect(state.error).toBe('HTTP error 404 not found');
    });

    it('should consume download promise rejections', async () => {
      mockAutoUpdater.downloadUpdate.mockRejectedValueOnce(
        new Error('Download failed')
      );
      const { init } = await import('@/main/update/index');
      init();

      autoUpdaterEventHandlers.get('update-available')?.({
        version: '2.0.0',
        releaseNotes: null,
      });
      await Promise.resolve();

      expect(mockAutoUpdater.downloadUpdate).toHaveBeenCalledTimes(1);
    });
  });

  describe('checkForUpdate', () => {
    it('should call autoUpdater.checkForUpdates', async () => {
      const { init, checkForUpdate } = await import('@/main/update/index');
      init();

      await checkForUpdate();

      expect(mockAutoUpdater.checkForUpdates).toHaveBeenCalled();
    });

    it('should return current update state', async () => {
      const { init, checkForUpdate } = await import('@/main/update/index');
      init();

      const state = await checkForUpdate();

      expect(state).toHaveProperty('status');
      expect(state).toHaveProperty('currentVersion');
    });

    it('should report a channel metadata 404', async () => {
      mockAutoUpdater.checkForUpdates.mockRejectedValueOnce(
        Object.assign(new Error('Not found'), {
          code: 'ERR_UPDATER_CHANNEL_FILE_NOT_FOUND',
        })
      );

      const { init, checkForUpdate, getUpdateState } =
        await import('@/main/update/index');
      init();

      await checkForUpdate();

      expect(getUpdateState().status).toBe('error');
      expect(getUpdateState().error).toBe('Failed to check for updates');
    });

    it('should not check for updates on Windows', async () => {
      mockPlatform.isMac = false;
      const { init, checkForUpdate, getUpdateState } =
        await import('@/main/update/index');
      init();

      await checkForUpdate();

      expect(getUpdateState().status).toBe('unsupported');
      expect(mockAutoUpdater.checkForUpdates).not.toHaveBeenCalled();
    });

    it('should use the dev update version and notes without the updater', async () => {
      process.env.PORATAKE_DEV_UPDATE_VERSION = '9.9.9';
      process.env.PORATAKE_DEV_UPDATE_NOTES = 'Dev build';

      const { init, checkForUpdate, getUpdateState } =
        await import('@/main/update/index');
      init();

      await checkForUpdate();

      expect(getUpdateState().latestVersion).toBe('9.9.9');
      expect(getUpdateState().releaseNotes).toBe('Dev build');
      expect(getUpdateState().status).toBe('ready');
      expect(mockAutoUpdater.checkForUpdates).not.toHaveBeenCalled();

      delete process.env.PORATAKE_DEV_UPDATE_VERSION;
      delete process.env.PORATAKE_DEV_UPDATE_NOTES;
    });

    it('should handle generic error', async () => {
      mockAutoUpdater.checkForUpdates.mockRejectedValueOnce(
        new Error('Network error')
      );

      const { init, checkForUpdate, getUpdateState } =
        await import('@/main/update/index');
      init();

      await checkForUpdate();

      const state = getUpdateState();
      expect(state.status).toBe('error');
      expect(state.error).toBe('Failed to check for updates');
    });
  });

  describe('installDownloadedUpdate', () => {
    it('should call quitAndInstall', async () => {
      const { init, installDownloadedUpdate } =
        await import('@/main/update/index');
      init();

      installDownloadedUpdate();

      expect(mockAutoUpdater.quitAndInstall).toHaveBeenCalledWith(false, true);
    });

    it('should not install updates on Windows', async () => {
      mockPlatform.isMac = false;
      const { init, installDownloadedUpdate, getUpdateState } =
        await import('@/main/update/index');
      init();

      installDownloadedUpdate();

      expect(getUpdateState().status).toBe('unsupported');
      expect(mockAutoUpdater.quitAndInstall).not.toHaveBeenCalled();
    });
  });

  describe('stopPeriodicUpdateChecks', () => {
    it('should stop periodic update checks', async () => {
      const { init, stopPeriodicUpdateChecks } =
        await import('@/main/update/index');
      init();

      // Wait for initial delay
      vi.advanceTimersByTime(3000);
      expect(mockAutoUpdater.checkForUpdates).toHaveBeenCalledTimes(1);

      stopPeriodicUpdateChecks();

      // Advance 30 minutes - should not trigger another check
      vi.advanceTimersByTime(30 * 60 * 1000);
      expect(mockAutoUpdater.checkForUpdates).toHaveBeenCalledTimes(1);
    });
  });

  describe('IPC handlers', () => {
    it('should handle update:getState', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      const handler = mockIpcMainHandlers.get('update:getState');
      const result = handler?.() as UpdateState;

      expect(result).toEqual(getUpdateState());
    });

    it('should handle update:check', async () => {
      const { init } = await import('@/main/update/index');
      init();

      const handler = mockIpcMainHandlers.get('update:check');
      await handler?.();

      expect(mockAutoUpdater.checkForUpdates).toHaveBeenCalled();
    });

    it('should handle update:install', async () => {
      const { init } = await import('@/main/update/index');
      init();

      const handler = mockIpcMainHandlers.get('update:install');
      handler?.();

      expect(mockAutoUpdater.quitAndInstall).toHaveBeenCalledWith(false, true);
    });
  });

  describe('Periodic checks skip conditions', () => {
    it('should skip periodic check when downloading', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      // Set status to downloading
      const progressHandler = autoUpdaterEventHandlers.get('download-progress');
      progressHandler?.({ percent: 50 });
      expect(getUpdateState().status).toBe('downloading');

      // Clear initial check
      vi.advanceTimersByTime(3000);
      mockAutoUpdater.checkForUpdates.mockClear();

      // Periodic check should be skipped
      vi.advanceTimersByTime(30 * 60 * 1000);
      expect(mockAutoUpdater.checkForUpdates).not.toHaveBeenCalled();
    });

    it('should skip periodic check when update is ready', async () => {
      const { init, getUpdateState } = await import('@/main/update/index');
      init();

      // Set status to ready
      const downloadedHandler =
        autoUpdaterEventHandlers.get('update-downloaded');
      downloadedHandler?.({ version: '2.0.0' });
      expect(getUpdateState().status).toBe('ready');

      // Clear initial check
      vi.advanceTimersByTime(3000);
      mockAutoUpdater.checkForUpdates.mockClear();

      // Periodic check should be skipped
      vi.advanceTimersByTime(30 * 60 * 1000);
      expect(mockAutoUpdater.checkForUpdates).not.toHaveBeenCalled();
    });
  });
});
