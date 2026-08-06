import { vi } from 'vitest';

/**
 * Mock Electron app module for testing.
 * Provides reasonable defaults for common app methods.
 */
export function createMockApp(overrides: Partial<Electron.App> = {}) {
  return {
    getVersion: vi.fn(() => '1.0.0'),
    getPath: vi.fn((name: string) => {
      const paths: Record<string, string> = {
        home: '/mock/home',
        userData: '/mock/home/.config/capty',
        pictures: '/mock/home/Pictures',
        videos: '/mock/home/Movies',
        temp: '/mock/tmp',
      };
      return paths[name] || `/mock/${name}`;
    }),
    isPackaged: false,
    ...overrides,
  } as unknown as Electron.App;
}

/**
 * Mock the entire electron module.
 */
export function mockElectron(appOverrides: Partial<Electron.App> = {}) {
  vi.mock('electron', () => ({
    app: createMockApp(appOverrides),
    ipcMain: {
      on: vi.fn(),
      handle: vi.fn(),
      removeHandler: vi.fn(),
    },
    BrowserWindow: vi.fn(),
    screen: {
      getPrimaryDisplay: vi.fn(() => ({
        scaleFactor: 2,
        workAreaSize: { width: 1920, height: 1080 },
      })),
    },
  }));
}
