import { ipcMain } from 'electron';
import { autoUpdater } from 'electron-updater';
import { existsSync } from 'fs';
import fs from 'fs/promises';
import path from 'path';

import type { UpdateState, UpdateStatus } from '@/types/update.ts';
import { getAppVersion, isDev } from '@/main/utils/env.ts';
import { getConfigDir } from '@/main/utils/paths.ts';
import { rebuildTrayMenu } from '@/main/menu';
import * as capture from '@/main/capture';
import { broadcastUpdateEvent } from './broadcast.ts';
import { API_URL } from './config.ts';

const INITIAL_CHECK_DELAY = 3000;
const UPDATE_CHECK_INTERVAL = 30 * 60 * 1000;

let updateCheckInterval: NodeJS.Timeout | null = null;

const updateState: UpdateState = {
  status: 'idle',
  currentVersion: '0.0.0',
  latestVersion: null,
  releaseNotes: null,
  downloadProgress: 0,
  downloadedFilePath: null,
  error: null,
};

export function getUpdateState(): UpdateState {
  return { ...updateState };
}

function setStatus(status: UpdateStatus): void {
  const previousStatus = updateState.status;
  updateState.status = status;
  broadcastUpdateEvent('update:status-changed', getUpdateState());

  const shouldRebuildTray =
    (status === 'ready' && previousStatus !== 'ready') ||
    (status === 'available' && previousStatus !== 'available');

  if (shouldRebuildTray) {
    rebuildTrayMenu();
  }
}

function setError(error: string): void {
  updateState.error = error;
  setStatus('error');
}

function configureAutoUpdater(): void {
  autoUpdater.setFeedURL({
    provider: 'generic',
    url: `${API_URL}/api/versions`,
  });

  autoUpdater.autoDownload = false;
  autoUpdater.autoInstallOnAppQuit = true;

  autoUpdater.on('checking-for-update', () => {
    setStatus('checking');
    updateState.error = null;
  });

  autoUpdater.on('update-available', info => {
    updateState.latestVersion = info.version;
    updateState.releaseNotes =
      typeof info.releaseNotes === 'string'
        ? info.releaseNotes
        : (info.releaseNotes?.[0]?.note ?? null);
    setStatus('available');

    autoUpdater.downloadUpdate();
  });

  autoUpdater.on('update-not-available', info => {
    updateState.latestVersion = info.version;
    setStatus('up_to_date');
  });

  autoUpdater.on('download-progress', progress => {
    const previousProgress = updateState.downloadProgress;
    updateState.downloadProgress = Math.round(progress.percent);
    setStatus('downloading');
    broadcastUpdateEvent('update:download-progress', progress.percent);

    if (
      Math.floor(updateState.downloadProgress / 10) !==
      Math.floor(previousProgress / 10)
    ) {
      rebuildTrayMenu();
    }
  });

  autoUpdater.on('update-downloaded', info => {
    updateState.latestVersion = info.version;
    updateState.downloadProgress = 100;
    setStatus('ready');
  });

  autoUpdater.on('error', (error: Error & { code?: string }) => {
    const isChannelNotFound =
      error.code === 'ERR_UPDATER_CHANNEL_FILE_NOT_FOUND' ||
      error.message?.includes('404');

    if (isChannelNotFound) {
      setStatus('up_to_date');
      return;
    }

    console.error('Auto-updater error:', error);
    setError(error.message || 'Update failed');
  });
}

export async function checkForUpdate(): Promise<UpdateState> {
  const devUpdateVersion = isDev
    ? process.env.CAPTY_DEV_UPDATE_VERSION
    : undefined;

  if (devUpdateVersion) {
    const currentVersion = getAppVersion();
    updateState.error = null;
    updateState.downloadedFilePath = null;
    updateState.latestVersion = devUpdateVersion;
    updateState.releaseNotes = process.env.CAPTY_DEV_UPDATE_NOTES || null;
    updateState.downloadProgress = 100;

    if (devUpdateVersion === currentVersion) {
      updateState.downloadProgress = 0;
      setStatus('up_to_date');
      return getUpdateState();
    }

    setStatus('ready');
    return getUpdateState();
  }

  try {
    await autoUpdater.checkForUpdates();
  } catch (error) {
    const err = error as Error & { code?: string };
    const isChannelNotFound =
      err.code === 'ERR_UPDATER_CHANNEL_FILE_NOT_FOUND' ||
      err.message?.includes('404');

    if (isChannelNotFound) {
      setStatus('up_to_date');
    } else {
      console.error('Update check failed:', error);
      setError('Failed to check for updates');
    }
  }
  return getUpdateState();
}

export function installDownloadedUpdate(): void {
  const devUpdateVersion = isDev
    ? process.env.CAPTY_DEV_UPDATE_VERSION
    : undefined;

  if (devUpdateVersion) {
    const currentVersion = getAppVersion();
    updateState.error = null;
    if (devUpdateVersion === currentVersion) {
      updateState.downloadProgress = 0;
      setStatus('up_to_date');
      return;
    }
    setStatus('ready');
    return;
  }
  autoUpdater.quitAndInstall(false, true);
}

function startPeriodicUpdateChecks(): void {
  if (updateCheckInterval) {
    clearInterval(updateCheckInterval);
  }

  updateCheckInterval = setInterval(() => {
    if (
      updateState.status !== 'downloading' &&
      updateState.status !== 'ready'
    ) {
      checkForUpdate();
    }
  }, UPDATE_CHECK_INTERVAL);
}

export function stopPeriodicUpdateChecks(): void {
  if (updateCheckInterval) {
    clearInterval(updateCheckInterval);
    updateCheckInterval = null;
  }
}

export function init(): void {
  updateState.currentVersion = getAppVersion();

  configureAutoUpdater();

  ipcMain.handle('update:getState', () => {
    return getUpdateState();
  });

  ipcMain.handle('update:check', async () => {
    return await checkForUpdate();
  });

  ipcMain.handle('update:install', () => {
    installDownloadedUpdate();
  });

  setTimeout(() => {
    checkForUpdate();
  }, INITIAL_CHECK_DELAY);

  startPeriodicUpdateChecks();
}

export type { UpdateState, UpdateStatus };

export async function handleAppUpdate(): Promise<void> {
  const versionFile = path.join(getConfigDir(), '.last-version');
  const currentVersion = getAppVersion();

  try {
    const configDir = getConfigDir();
    if (!existsSync(configDir)) {
      await fs.mkdir(configDir, { recursive: true });
    }

    let lastVersion: string | null = null;
    if (existsSync(versionFile)) {
      lastVersion = (await fs.readFile(versionFile, 'utf-8')).trim();
    }

    if (lastVersion !== currentVersion) {
      console.log(
        `App updated: ${lastVersion || 'first run'} -> ${currentVersion}`
      );
      capture.resetScreenCaptureCache();

      await fs.writeFile(versionFile, currentVersion, 'utf-8');
    }
  } catch (error) {
    console.error('Failed to check app version:', error);
    capture.resetScreenCaptureCache();
  }
}
