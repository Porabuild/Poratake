import { describe, it, expect, vi, beforeEach, afterAll } from 'vitest';

const mockRelease = vi.fn();

vi.mock('os', () => ({
  default: { release: () => mockRelease() },
  release: () => mockRelease(),
}));

const originalPlatform = process.platform;

function setPlatform(platform: string): void {
  Object.defineProperty(process, 'platform', {
    value: platform,
    configurable: true,
  });
}

async function loadPlatform(platform: string, release: string) {
  vi.resetModules();
  setPlatform(platform);
  mockRelease.mockReturnValue(release);
  return import('@/main/utils/platform');
}

describe('platform', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterAll(() => {
    setPlatform(originalPlatform);
  });

  it('detects the current platform', async () => {
    const { isWindows, isMac, isLinux } = await loadPlatform(
      'win32',
      '10.0.26200'
    );
    expect(isWindows).toBe(true);
    expect(isMac).toBe(false);
    expect(isLinux).toBe(false);
  });

  describe('supportsAcrylic', () => {
    it('is true on Windows 11 22H2', async () => {
      const { supportsAcrylic } = await loadPlatform('win32', '10.0.22621');
      expect(supportsAcrylic()).toBe(true);
    });

    it('is true on newer Windows 11 builds', async () => {
      const { supportsAcrylic } = await loadPlatform('win32', '10.0.26200');
      expect(supportsAcrylic()).toBe(true);
    });

    it('is false on Windows 11 builds before 22H2', async () => {
      const { supportsAcrylic } = await loadPlatform('win32', '10.0.22000');
      expect(supportsAcrylic()).toBe(false);
    });

    it('is false on Windows 10', async () => {
      const { supportsAcrylic } = await loadPlatform('win32', '10.0.19045');
      expect(supportsAcrylic()).toBe(false);
    });

    it('is false on macOS', async () => {
      const { supportsAcrylic } = await loadPlatform('darwin', '24.6.0');
      expect(supportsAcrylic()).toBe(false);
    });

    it('is false when the build number cannot be parsed', async () => {
      const { supportsAcrylic } = await loadPlatform('win32', 'unknown');
      expect(supportsAcrylic()).toBe(false);
    });
  });
});
