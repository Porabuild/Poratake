import type {
  ActivationResult,
  LicenseCache,
  ValidationResult,
} from '@/types/license.ts';
import { getAppVersion } from '@/main/utils/env.ts';
import { API_URL } from './config.ts';
import {
  generateDeviceFingerprint,
  getDeviceName,
  getDevicePlatform,
} from './device.ts';
import {
  clearLicenseCache,
  getCachedLicense,
  isOfflineCacheValid,
  saveLicenseCache,
  setCachedLicense,
  setLicenseStatus,
} from './cache.ts';
import { isVersionEntitled } from './validation.ts';

export async function activateLicense(
  email: string,
  licenseKey: string
): Promise<ActivationResult> {
  const deviceFingerprint = generateDeviceFingerprint();
  const deviceName = getDeviceName();
  const devicePlatform = getDevicePlatform();
  const appVersion = getAppVersion();

  try {
    const response = await fetch(`${API_URL}/api/license/activate`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
      },
      body: JSON.stringify({
        email,
        license_key: licenseKey,
        device_fingerprint: deviceFingerprint,
        device_name: deviceName,
        device_platform: devicePlatform,
        app_version: appVersion,
      }),
    });

    const data = await response.json();

    if (data.valid) {
      const cache: LicenseCache = {
        licenseKey,
        email,
        expiresAt: data.license.expires_at,
        maxVersion: data.license.max_version,
        isLifetime: data.license.is_lifetime,
        deviceFingerprint,
        lastValidated: new Date().toISOString(),
      };
      await saveLicenseCache(cache);
      setCachedLicense(cache);
      setLicenseStatus('valid');
    }

    return data;
  } catch (error) {
    console.error('License activation failed:', error);
    return {
      valid: false,
      error: 'network_error',
      message:
        'Unable to connect to license server. Please check your internet connection.',
    };
  }
}

export async function validateLicense(): Promise<ValidationResult> {
  const cachedLicense = getCachedLicense();

  if (!cachedLicense) {
    return { valid: false, error: 'not_activated' };
  }

  const deviceFingerprint = generateDeviceFingerprint();
  const appVersion = getAppVersion();

  try {
    const response = await fetch(`${API_URL}/api/license/validate`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
      },
      body: JSON.stringify({
        license_key: cachedLicense.licenseKey,
        device_fingerprint: deviceFingerprint,
        app_version: appVersion,
      }),
    });

    const data = await response.json();

    if (data.valid) {
      const updatedCache: LicenseCache = {
        ...cachedLicense,
        expiresAt: data.license.expires_at,
        maxVersion: data.license.max_version,
        isLifetime: data.license.is_lifetime,
        deviceFingerprint,
        lastValidated: new Date().toISOString(),
      };
      await saveLicenseCache(updatedCache);
      setCachedLicense(updatedCache);
      setLicenseStatus(data.license.is_expired ? 'expired' : 'valid');
    } else {
      if (data.error === 'device_not_activated') {
        setLicenseStatus('device_mismatch');
      } else {
        setLicenseStatus('invalid');
      }
    }

    return data;
  } catch {
    if (isOfflineCacheValid(cachedLicense)) {
      setLicenseStatus('offline_valid');
      return {
        valid: true,
        license: {
          expires_at: cachedLicense.expiresAt,
          is_expired: false,
          is_lifetime: cachedLicense.isLifetime,
          max_version: cachedLicense.maxVersion,
        },
        entitled_to_version: isVersionEntitled(cachedLicense.maxVersion),
      };
    } else {
      setLicenseStatus('offline_expired');
      return {
        valid: false,
        error: 'offline_cache_expired',
      };
    }
  }
}

export async function deactivateLicense(): Promise<boolean> {
  const cachedLicense = getCachedLicense();

  if (!cachedLicense) {
    return true;
  }

  const deviceFingerprint = generateDeviceFingerprint();

  try {
    await fetch(`${API_URL}/api/license/deactivate`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
      },
      body: JSON.stringify({
        license_key: cachedLicense.licenseKey,
        device_fingerprint: deviceFingerprint,
      }),
    });
  } catch {
    console.warn(
      'License deactivation request failed, proceeding to clear cache'
    );
  }

  await clearLicenseCache();
  return true;
}

export function getCheckoutUrl(): string {
  return `${API_URL}/#pricing`;
}

export function getLoginUrl(): string {
  return `${API_URL}/login`;
}
