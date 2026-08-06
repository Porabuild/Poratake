import { BrowserWindow, ipcMain } from 'electron';

import { initSSLSettings } from './config.ts';
import { isDev } from '@/main/utils/env.ts';
import {
  getCachedLicense,
  getLicenseStatus,
  loadCachedLicense,
  setCachedLicense,
  setLicenseStatus,
} from './cache.ts';
import { generateDeviceFingerprint } from './device.ts';
import { isPro, isFirstTimeActivation } from './validation.ts';
import {
  activateLicense,
  validateLicense,
  deactivateLicense,
  getCheckoutUrl,
  getLoginUrl,
} from './api.ts';

const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

function broadcastLicenseChanged(): void {
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.isDestroyed()) {
      window.webContents.send('license:changed');
    }
  }
}

async function checkLicense(): Promise<void> {
  const shouldCheckInDev = process.env.CHECK_LICENSE === 'true';
  if (isDev && !shouldCheckInDev) {
    return;
  }

  if (getCachedLicense()) {
    await validateLicense();
    broadcastLicenseChanged();
  }
}

export async function init(): Promise<void> {
  initSSLSettings();

  ipcMain.handle('license:getStatus', () => {
    const cachedLicense = getCachedLicense();
    return {
      status: getLicenseStatus(),
      info: cachedLicense
        ? {
            email: cachedLicense.email,
            expiresAt: cachedLicense.expiresAt,
            maxVersion: cachedLicense.maxVersion,
            isLifetime: cachedLicense.isLifetime,
          }
        : null,
    };
  });

  ipcMain.handle(
    'license:activate',
    async (_event, email: string, licenseKey: string) => {
      return await activateLicense(email, licenseKey);
    }
  );

  ipcMain.handle('license:validate', async () => {
    return await validateLicense();
  });

  ipcMain.handle('license:deactivate', async () => {
    return await deactivateLicense();
  });

  ipcMain.handle('license:isPro', () => {
    return isPro();
  });

  ipcMain.handle('license:getCheckoutUrl', () => {
    return getCheckoutUrl();
  });

  ipcMain.handle('license:getLoginUrl', () => {
    return getLoginUrl();
  });

  ipcMain.handle('license:isFirstTimeActivation', () => {
    return isFirstTimeActivation();
  });

  const cached = await loadCachedLicense();
  setCachedLicense(cached);

  if (!cached) {
    setLicenseStatus('not_activated');
  }

  await checkLicense();

  setInterval(checkLicense, CHECK_INTERVAL_MS);
}

export {
  activateLicense,
  validateLicense,
  deactivateLicense,
  getLicenseStatus,
  getCachedLicense as getLicenseInfo,
  isPro,
  isFirstTimeActivation,
  generateDeviceFingerprint,
};
