export function isMacPlatform(): boolean {
  return window.appPlatform === undefined || window.appPlatform === 'darwin';
}

export function isWindowsPlatform(): boolean {
  return window.appPlatform === 'win32';
}
