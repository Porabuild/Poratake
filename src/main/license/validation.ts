import { getAppVersion, isDev } from '@/main/utils/env.ts';
import { getLicenseStatus } from './cache.ts';

export function compareVersions(a: string, b: string): number {
  const partsA = a.split('.').map(Number);
  const partsB = b.split('.').map(Number);

  for (let i = 0; i < Math.max(partsA.length, partsB.length); i++) {
    const numA = partsA[i] || 0;
    const numB = partsB[i] || 0;

    if (numA < numB) return -1;
    if (numA > numB) return 1;
  }

  return 0;
}

export function isVersionEntitled(maxVersion: string | null): boolean {
  if (!maxVersion) {
    return true;
  }

  const currentVersion = getAppVersion();

  return compareVersions(currentVersion, maxVersion) <= 0;
}

export function isPro(): boolean {
  const shouldCheckInDev = process.env.CHECK_LICENSE === 'true';

  if (isDev && !shouldCheckInDev) {
    return true;
  }

  const status = getLicenseStatus();
  return status === 'valid' || status === 'offline_valid';
}

export function isFirstTimeActivation(): boolean {
  return getLicenseStatus() === 'not_activated';
}
