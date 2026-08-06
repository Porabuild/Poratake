import { isProduction } from '@/main/utils/env.ts';
import { getConfigDir, getLicenseFilePath } from '@/main/utils/paths.ts';

export const CONFIG_DIR = getConfigDir();
export const LICENSE_FILE = getLicenseFilePath();

export const API_URL = isProduction
  ? 'https://capty.app'
  : 'https://capty.test';

export function initSSLSettings(): void {
  if (isProduction) {
    process.env.NODE_TLS_REJECT_UNAUTHORIZED = '1';
  } else {
    process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0';
  }
}
