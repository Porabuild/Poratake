import * as crypto from 'crypto';
import * as os from 'os';
import * as fs from 'fs';
import { execSync } from 'child_process';

export function getMachineId(): string {
  try {
    if (os.platform() === 'darwin') {
      const output = execSync(
        "ioreg -rd1 -c IOPlatformExpertDevice | grep -E '(IOPlatformUUID)' | awk -F'\"' '{print $4}'"
      );
      return output.toString().trim();
    } else if (os.platform() === 'win32') {
      const output = execSync(
        'reg query HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Cryptography /v MachineGuid'
      );
      const match = output.toString().match(/REG_SZ\s+(.+)/);
      return match ? match[1].trim() : os.hostname();
    } else {
      if (fs.existsSync('/etc/machine-id')) {
        return fs.readFileSync('/etc/machine-id', 'utf8').trim();
      }
    }
  } catch {
    console.warn('Failed to retrieve machine ID, falling back to hostname');
  }

  return `${os.hostname()}-${os.platform()}-${os.arch()}`;
}

export function generateDeviceFingerprint(): string {
  const machineId = getMachineId();
  const platform = os.platform();

  const data = `${machineId}-${platform}`;
  return crypto
    .createHash('sha256')
    .update(data)
    .digest('hex')
    .substring(0, 32);
}

export function getDeviceName(): string {
  const hostname = os.hostname();
  return hostname.replace(/\.local$/, '');
}

export function getDevicePlatform(): string {
  const platform = os.platform();
  switch (platform) {
    case 'darwin':
      return 'macOS';
    case 'win32':
      return 'Windows';
    case 'linux':
      return 'Linux';
    default:
      return platform;
  }
}
