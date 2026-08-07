import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import path from 'path';

const mockApp = {
  getPath: vi.fn((name: string) => `/mock/${name}`),
  getAppPath: vi.fn(() => '/mock/app'),
  isPackaged: false,
};

const mockExistsSync = vi.fn((_path: string) => true);

vi.mock('fs', () => ({
  default: { existsSync: mockExistsSync },
  existsSync: mockExistsSync,
}));

vi.mock('electron', () => ({
  app: mockApp,
}));

describe('getNativeBinaryPath on Windows', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    vi.clearAllMocks();
    mockExistsSync.mockReturnValue(true);
    vi.resetModules();
    Object.defineProperty(process, 'platform', { value: 'win32' });
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
    vi.resetModules();
  });

  it('appends .exe to the daemon binary name', async () => {
    const { getNativeBinaryPath } = await import('@/main/utils/paths');
    expect(getNativeBinaryPath('capty-daemon')).toBe(
      path.join('/mock/app', 'src/main/daemon', 'capty-daemon.exe')
    );
  });

  it('appends .exe to other native binaries', async () => {
    const { getNativeBinaryPath } = await import('@/main/utils/paths');
    expect(getNativeBinaryPath('ffmpeg')).toBe(
      path.join('/mock/app', 'src/main/binaries/ffmpeg', 'ffmpeg.exe')
    );
  });
});
