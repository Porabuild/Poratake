import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import path from 'path';
import type { SettingsConfig } from '@/types/settings';

// Mock file system
const mockFs = {
  existsSync: vi.fn(),
  mkdirSync: vi.fn(),
  readFileSync: vi.fn(),
  writeFileSync: vi.fn(),
  renameSync: vi.fn(),
  readdirSync: vi.fn(() => []),
  unlinkSync: vi.fn(),
  copyFileSync: vi.fn(),
  promises: {
    writeFile: vi.fn().mockResolvedValue(undefined),
    rename: vi.fn().mockResolvedValue(undefined),
  },
};

vi.mock('fs', () => ({
  default: mockFs,
  existsSync: mockFs.existsSync,
  mkdirSync: mockFs.mkdirSync,
  readFileSync: mockFs.readFileSync,
  writeFileSync: mockFs.writeFileSync,
  renameSync: mockFs.renameSync,
  readdirSync: mockFs.readdirSync,
  unlinkSync: mockFs.unlinkSync,
  copyFileSync: mockFs.copyFileSync,
  promises: mockFs.promises,
}));

// Mock Electron
const mockApp = {
  on: vi.fn(),
  setLoginItemSettings: vi.fn(),
  getVersion: vi.fn(() => '1.0.0'),
  getPath: vi.fn((name: string) => {
    const paths: Record<string, string> = {
      home: '/mock/home',
    };
    return paths[name] || `/mock/${name}`;
  }),
};
const mockSafeStorage = {
  isEncryptionAvailable: vi.fn(() => true),
  encryptString: vi.fn((value: string) => Buffer.from(`encrypted:${value}`)),
  decryptString: vi.fn((value: Buffer) =>
    value.toString().replace(/^encrypted:/, '')
  ),
};

const mockIpcMain = {
  handle: vi.fn(),
};

vi.mock('electron', () => ({
  app: mockApp,
  BrowserWindow: {
    getAllWindows: () => [],
  },
  ipcMain: mockIpcMain,
  nativeTheme: {
    themeSource: 'system',
  },
  safeStorage: mockSafeStorage,
}));

// Mock utils/paths
vi.mock('@/main/utils/paths', () => ({
  getConfigDir: vi.fn(() => '/mock/home/.config/poratake-dev'),
  getConfigFilePath: vi.fn(() => '/mock/home/.config/poratake-dev/config.json'),
}));

vi.mock('@/main/settings/window', () => ({
  isSettingsWindowWebContents: () => true,
}));

describe('Config Management', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();

    // Default behavior - no config file exists
    mockFs.existsSync.mockReturnValue(false);
    mockApp.getPath.mockImplementation((name: string) => {
      const paths: Record<string, string> = { home: '/mock/home' };
      return paths[name] || `/mock/${name}`;
    });
    mockSafeStorage.isEncryptionAvailable.mockReturnValue(true);
    mockSafeStorage.encryptString.mockImplementation((value: string) =>
      Buffer.from(`encrypted:${value}`)
    );
    mockSafeStorage.decryptString.mockImplementation((value: Buffer) =>
      value.toString().replace(/^encrypted:/, '')
    );
    // Reset writeFileSync to default behavior
    mockFs.writeFileSync.mockImplementation(() => {});
  });

  afterEach(() => {
    vi.resetModules();
  });

  describe('loadConfig', () => {
    it('should return default settings when config file does not exist', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config).toEqual(DEFAULT_SETTINGS);
      expect(config).not.toBe(DEFAULT_SETTINGS);
      expect(config.wallpaper.customBackgrounds).not.toBe(
        DEFAULT_SETTINGS.wallpaper.customBackgrounds
      );
      expect(config.general.playSoundOnScreenshot).toBe(false);
      await vi.waitFor(() =>
        expect(mockFs.promises.writeFile).toHaveBeenCalled()
      );
    });

    it('should load config from file when it exists', async () => {
      const savedConfig: Partial<SettingsConfig> = {
        general: {
          startOnLogin: true,
          playSoundOnScreenshot: false,
        },
      };

      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(JSON.stringify(savedConfig));

      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config.general?.startOnLogin).toBe(true);
      expect(config.general?.playSoundOnScreenshot).toBe(false);
      expect(config.recording.autoZoom).toBe(false);
      expect(config.appearance).toEqual({ mode: 'dark', theme: 'default' });
      expect(config.storage.screenshotsPath).toBe(
        path.join('/mock/pictures', 'Poratake')
      );
      expect(config.storage.recordingsPath).toBe(
        path.join('/mock/videos', 'Poratake')
      );
    });

    it('should migrate plaintext cloud secrets to OS-protected values', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({
          cloud: {
            enabled: true,
            activeProvider: 'rest',
            s3: { secretAccessKey: 's3-secret' },
            rest: {
              url: 'https://example.com/upload',
              headers: [{ key: 'Authorization', value: 'Bearer secret' }],
            },
          },
        })
      );

      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config.cloud.s3.secretAccessKey).toBe('s3-secret');
      expect(config.cloud.rest.headers[0].value).toBe('Bearer secret');
      await vi.waitFor(() =>
        expect(mockFs.promises.writeFile).toHaveBeenCalled()
      );

      const serialized = mockFs.promises.writeFile.mock.calls.at(
        -1
      )?.[1] as string;
      const persisted = JSON.parse(serialized) as SettingsConfig;
      expect(serialized).not.toContain('s3-secret');
      expect(serialized).not.toContain('Bearer secret');
      expect(persisted.cloud.s3.secretAccessKey).toMatch(
        /^poratake-safe-storage:v1:/
      );
      expect(persisted.cloud.rest.headers[0].value).toMatch(
        /^poratake-safe-storage:v1:/
      );
    });

    it('should preserve encrypted credentials and unrelated settings when OS encryption is unavailable', async () => {
      const storedSecret = `poratake-safe-storage:v1:${Buffer.from('encrypted:s3-secret').toString('base64')}`;
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({
          general: { startOnLogin: true },
          cloud: { s3: { secretAccessKey: storedSecret } },
        })
      );
      mockSafeStorage.isEncryptionAvailable.mockReturnValue(false);
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      const { loadConfig, updateConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config.general.startOnLogin).toBe(true);
      expect(config.cloud.s3.secretAccessKey).toBe('');

      updateConfig({ screenshot: { format: 'jpeg' } });
      await vi.waitFor(() =>
        expect(mockFs.promises.writeFile).toHaveBeenCalled()
      );
      const serialized = mockFs.promises.writeFile.mock.calls.at(
        -1
      )?.[1] as string;
      const persisted = JSON.parse(serialized) as SettingsConfig;
      expect(persisted.general.startOnLogin).toBe(true);
      expect(persisted.screenshot.format).toBe('jpeg');
      expect(persisted.cloud.s3.secretAccessKey).toBe(storedSecret);

      errorSpy.mockRestore();
    });

    it('should preserve settings when an encrypted credential cannot be decrypted', async () => {
      const storedSecret = `poratake-safe-storage:v1:${Buffer.from('invalid').toString('base64')}`;
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({
          general: { startOnLogin: true },
          cloud: { s3: { secretAccessKey: storedSecret } },
        })
      );
      mockSafeStorage.decryptString.mockImplementation(() => {
        throw new Error('decrypt failed');
      });
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config.general.startOnLogin).toBe(true);
      expect(config.cloud.s3.secretAccessKey).toBe('');
      expect(errorSpy).toHaveBeenCalledWith(
        'Failed to reveal cloud credential:',
        expect.any(Error)
      );

      errorSpy.mockRestore();
    });

    it('should preserve valid settings around malformed wallpaper entries', async () => {
      const storedSecret = `poratake-safe-storage:v1:${Buffer.from('encrypted:secret').toString('base64')}`;
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({
          general: { startOnLogin: true },
          cloud: { s3: { secretAccessKey: storedSecret } },
          wallpaper: {
            customBackgrounds: [{ id: 'bad', type: 'image' }],
            presets: [{ id: 'bad', backgroundImage: 42 }],
          },
        })
      );

      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config.general.startOnLogin).toBe(true);
      expect(config.cloud.s3.secretAccessKey).toBe('secret');
    });

    it('should keep the saved default wallpaper preset', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({
          wallpaper: {
            customBackgrounds: [],
            presets: [{ id: 'p1', name: 'Mine' }],
            defaultPresetId: 'p1',
          },
        })
      );

      const { loadConfig } = await import('@/main/settings');

      expect(loadConfig().wallpaper.defaultPresetId).toBe('p1');
    });

    it('should migrate wallpaper image data out of config JSON', async () => {
      const imageData = `data:image/png;base64,${Buffer.from('wallpaper').toString('base64')}`;
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({
          wallpaper: {
            customBackgrounds: [
              { id: 'image-1', type: 'image', data: { imageUrl: imageData } },
            ],
            presets: [
              {
                id: 'preset-1',
                name: 'Image',
                gradient: null,
                backgroundImage: imageData,
                padding: 10,
                corners: 10,
                shadow: 10,
              },
            ],
          },
        })
      );

      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config.wallpaper.customBackgrounds[0]).toMatchObject({
        type: 'image',
        data: { imageUrl: expect.stringMatching(/^file:/) },
      });
      expect(config.wallpaper.presets[0].backgroundImage).toMatch(/^file:/);
      await vi.waitFor(() =>
        expect(mockFs.promises.writeFile).toHaveBeenCalled()
      );
      const serialized = mockFs.promises.writeFile.mock.calls.at(
        -1
      )?.[1] as string;
      expect(serialized).not.toContain('data:image');
      expect(serialized).not.toContain(
        Buffer.from('wallpaper').toString('base64')
      );
    });

    it('should drop a default wallpaper preset that no longer exists', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({
          wallpaper: {
            customBackgrounds: [{ id: 'bg1', type: 'gradient', data: {} }],
            presets: [],
            defaultPresetId: 'gone',
          },
        })
      );

      const { loadConfig } = await import('@/main/settings');

      expect(loadConfig().wallpaper.defaultPresetId).toBeNull();
    });

    it('should normalize invalid saved appearance values', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({ appearance: { mode: 'invalid', theme: 'missing' } })
      );

      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config.appearance).toEqual({ mode: 'dark', theme: 'default' });
    });

    it('should merge saved config with defaults for new settings', async () => {
      // Simulate old config without recording settings
      const oldConfig = {
        general: {
          startOnLogin: false,
          playSoundOnScreenshot: true,
          showNotifications: true,
        },
        screenshot: {
          hideDesktopIcons: false,
          captureToClipboard: false,
          includeCursor: true,
        },
      };

      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(JSON.stringify(oldConfig));

      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      // Should have the old settings
      expect(config.general.startOnLogin).toBe(false);
      expect(config.screenshot.hideDesktopIcons).toBe(false);
      // Should also have new default settings
      expect(config.screenshot.autoCopyToClipboard).toBe(true);
      expect(config.recording).toBeDefined();
      expect(config.recording.autoZoom).toBe(false);
      expect(config.editor).toBeDefined();
    });

    it('should initialize All-in-One inputs from existing recording preferences', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({
          recording: {
            systemAudio: false,
            micEnabled: true,
            camera: { enabled: true },
          },
          allInOne: {
            lastArea: { x: 10, y: 20, width: 300, height: 200 },
          },
        })
      );

      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config.allInOne).toEqual(
        expect.objectContaining({
          rememberChoices: true,
          lastMode: 'screenshot',
          lastTargets: { screenshot: 'area', record: 'area' },
          lastArea: { x: 10, y: 20, width: 300, height: 200 },
          recording: {
            systemAudio: false,
            micEnabled: true,
            cameraEnabled: true,
          },
        })
      );
    });

    it('should return defaults on parse error', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue('invalid json');

      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config).toEqual(DEFAULT_SETTINGS);
      expect(mockFs.renameSync).toHaveBeenCalledWith(
        '/mock/home/.config/poratake-dev/config.json',
        expect.stringMatching(/config\.corrupt-\d+\.json$/)
      );
    });

    it('should sanitize structurally invalid config fields', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({
          general: { startOnLogin: true },
          wallpaper: {
            customBackgrounds: [],
            presets: [],
            customGradients: {},
          },
          cloud: { s3: { secretAccessKey: 42 } },
        })
      );

      const { loadConfig } = await import('@/main/settings');
      const config = loadConfig();

      expect(config.general.startOnLogin).toBe(true);
      expect(config.wallpaper.customBackgrounds).toEqual([]);
      expect(config.wallpaper.presets).toEqual([]);
      expect(config.cloud.s3.secretAccessKey).toBe('');
    });

    it('should preserve a failed migration without plaintext cloud secrets', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(
        JSON.stringify({
          general: { startOnLogin: true },
          cloud: { s3: { secretAccessKey: 'plain-secret' } },
        })
      );
      mockApp.getPath.mockImplementation((name: string) => {
        if (name === 'pictures') {
          throw new Error('storage failure');
        }
        return `/mock/${name}`;
      });
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      const { loadConfig } = await import('@/main/settings');
      expect(loadConfig()).toEqual(DEFAULT_SETTINGS);
      const recoveryWrite = mockFs.writeFileSync.mock.calls.find(([filePath]) =>
        /config\.corrupt-\d+\.json$/.test(String(filePath))
      );
      expect(recoveryWrite).toBeDefined();
      expect(String(recoveryWrite?.[1])).not.toContain('plain-secret');
      expect(JSON.parse(String(recoveryWrite?.[1]))).not.toHaveProperty(
        'cloud'
      );
      expect(mockFs.unlinkSync).toHaveBeenCalledWith(
        '/mock/home/.config/poratake-dev/config.json'
      );
      errorSpy.mockRestore();
    });

    it('should create config directory if it does not exist', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { loadConfig } = await import('@/main/settings');
      loadConfig();

      expect(mockFs.mkdirSync).toHaveBeenCalledWith(
        '/mock/home/.config/poratake-dev',
        { recursive: true }
      );
    });
  });

  describe('saveConfig', () => {
    it('should save config to file', async () => {
      mockFs.existsSync.mockReturnValue(true);

      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      const { saveConfig } = await import('@/main/settings');

      saveConfig(DEFAULT_SETTINGS);

      await vi.waitFor(() =>
        expect(mockFs.promises.rename).toHaveBeenCalledWith(
          expect.stringMatching(/config\.json\.\d+\.\d+\.tmp$/),
          '/mock/home/.config/poratake-dev/config.json'
        )
      );
      expect(mockFs.promises.writeFile).toHaveBeenCalledWith(
        expect.stringMatching(/config\.json\.\d+\.\d+\.tmp$/),
        JSON.stringify(DEFAULT_SETTINGS, null, 2),
        'utf-8'
      );
    });

    it('should create directory before saving', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      const { saveConfig } = await import('@/main/settings');

      saveConfig(DEFAULT_SETTINGS);

      await vi.waitFor(() =>
        expect(mockFs.promises.writeFile).toHaveBeenCalled()
      );
      expect(mockFs.mkdirSync).toHaveBeenCalledWith(
        '/mock/home/.config/poratake-dev',
        { recursive: true }
      );
    });

    it('should log errors instead of throwing when saving fails', async () => {
      mockFs.existsSync.mockReturnValue(true);
      mockFs.promises.writeFile.mockRejectedValueOnce(
        new Error('Permission denied')
      );
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      const { saveConfig, getConfig } = await import('@/main/settings');

      expect(() => saveConfig(DEFAULT_SETTINGS)).not.toThrow(
        'Permission denied'
      );
      await vi.waitFor(() =>
        expect(errorSpy).toHaveBeenCalledWith(
          'Failed to save config:',
          expect.any(Error)
        )
      );
      mockFs.existsSync.mockReturnValue(false);
      expect(getConfig()).toEqual(DEFAULT_SETTINGS);

      errorSpy.mockRestore();
    });
  });

  describe('getConfig', () => {
    it('should return current config', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      const { loadConfig, getConfig } = await import('@/main/settings');

      loadConfig();
      const config = getConfig();

      expect(config).toEqual(DEFAULT_SETTINGS);
    });
  });

  describe('updateConfig', () => {
    it('should update config with partial updates', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { loadConfig, updateConfig, getConfig } =
        await import('@/main/settings');

      loadConfig();

      const updates: Partial<SettingsConfig> = {
        general: {
          startOnLogin: true,
          playSoundOnScreenshot: false,
        },
      };

      updateConfig(updates);
      const config = getConfig();

      expect(config.general.startOnLogin).toBe(true);
      expect(config.general.playSoundOnScreenshot).toBe(false);
      await vi.waitFor(() =>
        expect(mockFs.promises.writeFile).toHaveBeenCalled()
      );
    });

    it('should merge nested objects correctly', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { loadConfig, updateConfig, getConfig } =
        await import('@/main/settings');

      loadConfig();

      // Update only some screenshot settings
      updateConfig({
        screenshot: {
          closeOnCopy: false,
          closeOnSave: false,
          captureToClipboard: false,
          hideDesktopIcons: true,
        },
      });

      const config = getConfig();

      expect(config.screenshot.hideDesktopIcons).toBe(true);
      // Other screenshot settings should remain at defaults
      expect(config.screenshot.captureToClipboard).toBeDefined();

      updateConfig({
        appearance: { mode: 'light', theme: 'github' },
      });

      expect(getConfig().appearance).toEqual({
        mode: 'light',
        theme: 'github',
      });
    });

    it('should update login item settings when startOnLogin changes', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { loadConfig, updateConfig } = await import('@/main/settings');

      loadConfig();

      updateConfig({
        general: {
          startOnLogin: true,
          playSoundOnScreenshot: true,
        },
      });

      expect(mockApp.setLoginItemSettings).toHaveBeenCalledWith({
        openAtLogin: true,
      });
    });

    it('should handle login item setting errors gracefully', async () => {
      mockFs.existsSync.mockReturnValue(false);
      mockApp.setLoginItemSettings.mockImplementation(() => {
        throw new Error('Platform not supported');
      });

      const { loadConfig, updateConfig } = await import('@/main/settings');

      loadConfig();

      // Should not throw
      expect(() =>
        updateConfig({
          general: {
            startOnLogin: true,
            playSoundOnScreenshot: true,
          },
        })
      ).not.toThrow();
    });
  });

  describe('needsOnboarding', () => {
    it('should return true when onboarding not completed and not skipped', async () => {
      const savedConfig = {
        onboarding: { completed: false, skipped: false },
      };

      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(JSON.stringify(savedConfig));

      const { loadConfig, needsOnboarding } = await import('@/main/settings');

      loadConfig();
      expect(needsOnboarding()).toBe(true);
    });

    it('should return false when onboarding completed', async () => {
      const savedConfig = {
        onboarding: { completed: true, skipped: false },
      };

      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(JSON.stringify(savedConfig));

      const { loadConfig, needsOnboarding } = await import('@/main/settings');

      loadConfig();
      expect(needsOnboarding()).toBe(false);
    });

    it('should return false when onboarding skipped', async () => {
      const savedConfig = {
        onboarding: { completed: false, skipped: true },
      };

      mockFs.existsSync.mockReturnValue(true);
      mockFs.readFileSync.mockReturnValue(JSON.stringify(savedConfig));

      const { loadConfig, needsOnboarding } = await import('@/main/settings');

      loadConfig();
      expect(needsOnboarding()).toBe(false);
    });
  });

  describe('markOnboardingCompleted', () => {
    it('should set onboarding completed to true', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { loadConfig, markOnboardingCompleted, getConfig } =
        await import('@/main/settings');

      loadConfig();
      markOnboardingCompleted();

      const config = getConfig();
      expect(config.onboarding.completed).toBe(true);
      expect(config.onboarding.skipped).toBe(false);
    });
  });

  describe('markOnboardingSkipped', () => {
    it('should set onboarding skipped to true', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { loadConfig, markOnboardingSkipped, getConfig } =
        await import('@/main/settings');

      loadConfig();
      markOnboardingSkipped();

      const config = getConfig();
      expect(config.onboarding.skipped).toBe(true);
      expect(config.onboarding.completed).toBe(false);
    });
  });

  describe('init', () => {
    let ipcHandlers: Record<string, (...args: unknown[]) => unknown>;

    beforeEach(() => {
      ipcHandlers = {};
      mockIpcMain.handle.mockImplementation(
        (channel: string, handler: (...args: unknown[]) => unknown) => {
          ipcHandlers[channel] = handler;
        }
      );
    });

    it('should register all IPC handlers', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'settings:get-ui',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'settings:update',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'settings:reset',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'app:getVersion',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'editor:getPreferences',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'editor:updatePreferences',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'wallpaper:getSettings',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'wallpaper:addBackground',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'wallpaper:updateBackground',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'wallpaper:deleteBackground',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'wallpaper:addPreset',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'wallpaper:updatePreset',
        expect.any(Function)
      );
      expect(mockIpcMain.handle).toHaveBeenCalledWith(
        'wallpaper:deletePreset',
        expect.any(Function)
      );
    });

    it('should handle settings:get-ui IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const handler = ipcHandlers['settings:get-ui'];
      const result = handler({ sender: {} });

      expect(result).toHaveProperty('screenshot');
      expect(result).not.toHaveProperty('wallpaper');
    });

    it('should handle settings:update IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const handler = ipcHandlers['settings:update'];
      const result = handler(
        { sender: {} },
        { general: { startOnLogin: true } }
      );

      expect(result.general.startOnLogin).toBe(true);
    });

    it('should handle settings:reset IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      init();

      // First update settings
      const updateHandler = ipcHandlers['settings:update'];
      updateHandler({ sender: {} }, { general: { startOnLogin: true } });

      // Then reset
      const resetHandler = ipcHandlers['settings:reset'];
      const result = resetHandler({ sender: {} });

      expect(result).toEqual(DEFAULT_SETTINGS);
    });

    it('should handle app:getVersion IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const handler = ipcHandlers['app:getVersion'];
      const result = handler();

      expect(result).toBe('1.0.0');
    });

    it('should handle editor:getPreferences IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const handler = ipcHandlers['editor:getPreferences'];
      const result = handler();

      expect(result).toBeDefined();
    });

    it('should handle editor:updatePreferences IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const handler = ipcHandlers['editor:updatePreferences'];
      const result = handler({}, { fontSize: 16 });

      expect(result).toBeDefined();
    });

    it('should handle wallpaper:getSettings IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const handler = ipcHandlers['wallpaper:getSettings'];
      const result = handler();

      expect(result).toBeDefined();
      expect(result.customBackgrounds).toBeDefined();
      expect(result.presets).toBeDefined();
    });

    it('should handle wallpaper:addBackground IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      init();

      const background = {
        id: 'test-background',
        type: 'gradient' as const,
        data: {
          colors: ['#FF0000', '#00FF00'],
          angle: 45,
        },
      };

      const handler = ipcHandlers['wallpaper:addBackground'];
      const result = handler({}, background);

      expect(result).toContainEqual(background);
      expect(DEFAULT_SETTINGS.wallpaper.customBackgrounds).toEqual([]);
    });

    it('should handle wallpaper:updateBackground IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const background = {
        id: 'test-background',
        type: 'gradient' as const,
        data: {
          colors: ['#FF0000', '#00FF00'],
          angle: 45,
        },
      };

      const addHandler = ipcHandlers['wallpaper:addBackground'];
      addHandler({}, background);

      const updatedBackground = {
        ...background,
        data: { ...background.data, angle: 90 },
      };
      const updateHandler = ipcHandlers['wallpaper:updateBackground'];
      const result = updateHandler({}, updatedBackground);

      expect(
        result.find(
          (b: { id: string; data: { angle: number } }) =>
            b.id === 'test-background'
        )?.data.angle
      ).toBe(90);
    });

    it('should handle wallpaper:deleteBackground IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const background = {
        id: 'test-background',
        type: 'gradient' as const,
        data: {
          colors: ['#FF0000', '#00FF00'],
          angle: 45,
        },
      };

      const addHandler = ipcHandlers['wallpaper:addBackground'];
      addHandler({}, background);

      const deleteHandler = ipcHandlers['wallpaper:deleteBackground'];
      const result = deleteHandler({}, 'test-background');

      expect(
        result.find((b: { id: string }) => b.id === 'test-background')
      ).toBeUndefined();
    });

    it('should handle wallpaper:addPreset IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const preset = {
        id: 'test-preset',
        name: 'Test Preset',
        wallpaper: {},
      };

      const handler = ipcHandlers['wallpaper:addPreset'];
      const result = handler({}, preset);

      expect(result).toContainEqual(preset);
    });

    it('should handle wallpaper:updatePreset IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const preset = {
        id: 'test-preset',
        name: 'Test Preset',
        wallpaper: {},
      };

      const addHandler = ipcHandlers['wallpaper:addPreset'];
      addHandler({}, preset);

      const updatedPreset = { ...preset, name: 'Updated Preset' };
      const updateHandler = ipcHandlers['wallpaper:updatePreset'];
      const result = updateHandler({}, updatedPreset);

      expect(
        result.find((p: { id: string; name: string }) => p.id === 'test-preset')
          ?.name
      ).toBe('Updated Preset');
    });

    it('should handle wallpaper:deletePreset IPC call', async () => {
      mockFs.existsSync.mockReturnValue(false);

      const { init } = await import('@/main/settings');
      init();

      const preset = {
        id: 'test-preset',
        name: 'Test Preset',
        wallpaper: {},
      };

      const addHandler = ipcHandlers['wallpaper:addPreset'];
      addHandler({}, preset);

      const deleteHandler = ipcHandlers['wallpaper:deletePreset'];
      const result = deleteHandler({}, 'test-preset');

      expect(
        result.find((p: { id: string }) => p.id === 'test-preset')
      ).toBeUndefined();
    });
  });

  it('flushes pending writes synchronously on quit', async () => {
    mockFs.existsSync.mockReturnValue(false);

    const { DEFAULT_SETTINGS } = await import('@/types/settings');
    const { init, saveConfig } = await import('@/main/settings');
    init();

    const quitHandler = mockApp.on.mock.calls.find(
      call => call[0] === 'will-quit'
    )?.[1] as () => void;
    expect(quitHandler).toBeInstanceOf(Function);

    saveConfig(DEFAULT_SETTINGS);
    quitHandler();

    expect(mockFs.writeFileSync).toHaveBeenCalledWith(
      '/mock/home/.config/poratake-dev/config.json.flush.tmp',
      JSON.stringify(DEFAULT_SETTINGS, null, 2),
      'utf-8'
    );
    expect(mockFs.renameSync).toHaveBeenCalledWith(
      '/mock/home/.config/poratake-dev/config.json.flush.tmp',
      '/mock/home/.config/poratake-dev/config.json'
    );
  });
});
