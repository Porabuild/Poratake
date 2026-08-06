export interface LicenseInfo {
  key: string;
  email: string;
  purchasedAt: string;
  expiresAt: string;
  maxVersion: string | null;
  isExpired: boolean;
}

export interface DeviceInfo {
  id: string;
  name: string;
  activatedAt: string;
}

export interface LicenseCache {
  licenseKey: string;
  email: string;
  expiresAt: string | null;
  maxVersion: string | null;
  isLifetime: boolean;
  deviceFingerprint: string;
  lastValidated: string;
}

export interface ActivationResult {
  valid: boolean;
  license?: {
    key: string;
    purchased_at: string;
    expires_at: string | null;
    is_lifetime: boolean;
    max_version: string | null;
  };
  device?: {
    id: number;
    name: string;
    activated_at: string;
  };
  error?: string;
  message?: string;
}

export interface ValidationResult {
  valid: boolean;
  license?: {
    expires_at: string | null;
    is_expired: boolean;
    is_lifetime: boolean;
    max_version: string | null;
  };
  entitled_to_version?: boolean;
  error?: string;
}

export type LicenseStatus =
  | 'valid'
  | 'expired'
  | 'invalid'
  | 'not_activated'
  | 'device_mismatch'
  | 'offline_valid'
  | 'offline_expired';
