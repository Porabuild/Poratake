import os from 'os';

export const isMac = process.platform === 'darwin';
export const isWindows = process.platform === 'win32';
export const isLinux = process.platform === 'linux';

const WINDOWS_ACRYLIC_MIN_BUILD = 22621;

function getWindowsBuild(): number {
  return Number(os.release().split('.')[2]) || 0;
}

export function supportsAcrylic(): boolean {
  return isWindows && getWindowsBuild() >= WINDOWS_ACRYLIC_MIN_BUILD;
}
