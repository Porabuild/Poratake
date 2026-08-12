import { describe, it, expect, vi, afterAll } from 'vitest';

const originalPlatform = process.platform;

function setPlatform(platform: string): void {
  Object.defineProperty(process, 'platform', {
    value: platform,
    configurable: true,
  });
}

async function loadPlatform(platform: string) {
  vi.resetModules();
  setPlatform(platform);
  return import('@/main/utils/platform');
}

describe('platform', () => {
  afterAll(() => {
    setPlatform(originalPlatform);
  });

  it('detects Windows', async () => {
    const { isWindows, isMac, isLinux } = await loadPlatform('win32');
    expect(isWindows).toBe(true);
    expect(isMac).toBe(false);
    expect(isLinux).toBe(false);
  });

  it('detects macOS', async () => {
    const { isWindows, isMac, isLinux } = await loadPlatform('darwin');
    expect(isMac).toBe(true);
    expect(isWindows).toBe(false);
    expect(isLinux).toBe(false);
  });
});
