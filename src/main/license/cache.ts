import * as fs from 'fs/promises';
import { existsSync, mkdirSync } from 'fs';

import type { LicenseCache, LicenseStatus } from '@/types/license.ts';
import { CONFIG_DIR, LICENSE_FILE } from './config.ts';
import { generateDeviceFingerprint } from './device.ts';

let cachedLicense: LicenseCache | null = null;
let licenseStatus: LicenseStatus = 'not_activated';

export async function loadCachedLicense(): Promise<LicenseCache | null> {
  try {
    if (existsSync(LICENSE_FILE)) {
      const data = await fs.readFile(LICENSE_FILE, 'utf8');
      return JSON.parse(data);
    }
  } catch (error) {
    console.error('Failed to load cached license:', error);
  }
  return null;
}

export async function saveLicenseCache(cache: LicenseCache): Promise<void> {
  try {
    if (!existsSync(CONFIG_DIR)) {
      mkdirSync(CONFIG_DIR, { recursive: true });
    }
    await fs.writeFile(LICENSE_FILE, JSON.stringify(cache, null, 2));
  } catch (error) {
    console.error('Failed to save license cache:', error);
  }
}

export async function clearLicenseCache(): Promise<void> {
  try {
    if (existsSync(LICENSE_FILE)) {
      await fs.unlink(LICENSE_FILE);
    }
    cachedLicense = null;
    licenseStatus = 'not_activated';
  } catch (error) {
    console.error('Failed to clear license cache:', error);
  }
}

export function isOfflineCacheValid(cache: LicenseCache): boolean {
  const now = new Date();

  if (cache.deviceFingerprint !== generateDeviceFingerprint()) {
    return false;
  }

  if (cache.isLifetime) {
    return true;
  }

  if (cache.expiresAt) {
    const expiresAt = new Date(cache.expiresAt);
    if (expiresAt < now) {
      return false;
    }
  }

  return true;
}

export function getCachedLicense(): LicenseCache | null {
  return cachedLicense;
}

export function setCachedLicense(cache: LicenseCache | null): void {
  cachedLicense = cache;
}

export function getLicenseStatus(): LicenseStatus {
  return licenseStatus;
}

export function setLicenseStatus(status: LicenseStatus): void {
  licenseStatus = status;
}
