import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'path';
import { fileURLToPath, pathToFileURL } from 'url';
type Handler = (...args: unknown[]) => unknown;
const ipcHandle: Record<string, Handler> = {};

const mockIpcHandle = vi.fn((e: string, h: Handler) => {
  ipcHandle[e] = h;
});

const mockShowOpenDialog = vi.fn();
const mockReadFileSync = vi.fn();
const mockWriteFileSync = vi.fn();
const mockExistsSync = vi.fn();
const mockMkdirSync = vi.fn();
const mockStatSync = vi.fn();
const mockUnlinkSync = vi.fn();
const mockReaddirSync = vi.fn(() => []);
const mockCopyFileSync = vi.fn();
const mockDaemonCall = vi.fn();
const mockFromWebContents = vi.fn();
const mockIsSettingsWindowWebContents = vi.fn();
const mockSafeStorage = {
  isEncryptionAvailable: vi.fn(() => true),
  encryptString: vi.fn((value: string) => Buffer.from(`encrypted:${value}`)),
  decryptString: vi.fn((value: Buffer) =>
    value.toString().replace(/^encrypted:/, '')
  ),
};

vi.mock('electron', () => ({
  app: {
    on: vi.fn(),
    setLoginItemSettings: vi.fn(),
    isPackaged: false,
    getVersion: () => '1.0.0',
    getPath: (name: string) => {
      const paths: Record<string, string> = {
        home: '/home',
        pictures: '/home/Pictures',
        videos: '/home/Movies',
        userData: '/home/.config/poratake',
      };
      return paths[name] || '/tmp';
    },
  },
  BrowserWindow: {
    getAllWindows: () => [],
    fromWebContents: (...args: unknown[]) => mockFromWebContents(...args),
  },
  dialog: {
    showOpenDialog: (...a: unknown[]) => mockShowOpenDialog(...a),
  },
  ipcMain: {
    on: vi.fn(),
    handle: (e: string, h: Handler) => mockIpcHandle(e, h),
  },
  nativeTheme: {
    themeSource: 'system',
  },
  safeStorage: mockSafeStorage,
}));

vi.mock('fs', () => ({
  default: {
    existsSync: (...a: unknown[]) => mockExistsSync(...a),
    readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
    writeFileSync: (...a: unknown[]) => mockWriteFileSync(...a),
    mkdirSync: (...a: unknown[]) => mockMkdirSync(...a),
    statSync: (...a: unknown[]) => mockStatSync(...a),
    unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
    readdirSync: (...a: unknown[]) => mockReaddirSync(...a),
    copyFileSync: (...a: unknown[]) => mockCopyFileSync(...a),
  },
  existsSync: (...a: unknown[]) => mockExistsSync(...a),
  readFileSync: (...a: unknown[]) => mockReadFileSync(...a),
  writeFileSync: (...a: unknown[]) => mockWriteFileSync(...a),
  mkdirSync: (...a: unknown[]) => mockMkdirSync(...a),
  statSync: (...a: unknown[]) => mockStatSync(...a),
  unlinkSync: (...a: unknown[]) => mockUnlinkSync(...a),
  readdirSync: (...a: unknown[]) => mockReaddirSync(...a),
  copyFileSync: (...a: unknown[]) => mockCopyFileSync(...a),
}));

vi.mock('@/main/daemon', () => ({
  daemon: { call: (...a: unknown[]) => mockDaemonCall(...a) },
}));

vi.mock('@/main/settings/window', () => ({
  isSettingsWindowWebContents: (...args: unknown[]) =>
    mockIsSettingsWindowWebContents(...args),
}));

vi.mock('@/main/system/permissions', () => ({
  init: vi.fn(),
}));

describe('settings IPC handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFromWebContents.mockReset();
    mockIsSettingsWindowWebContents.mockReturnValue(true);
    vi.resetModules();
    Object.keys(ipcHandle).forEach(k => delete ipcHandle[k]);
    mockReadFileSync.mockReturnValue(Buffer.from('image-bytes'));
    mockReaddirSync.mockReturnValue([]);
    mockExistsSync.mockReturnValue(true);
    mockStatSync.mockReturnValue({ isDirectory: () => true });
    mockSafeStorage.isEncryptionAvailable.mockReturnValue(true);
  });

  async function loadAndInit() {
    const m = await import('@/main/settings');
    m.init();
    return m;
  }

  describe('settings:get / update / reset', () => {
    it('omits wallpaper data from renderer settings', async () => {
      await loadAndInit();
      const result = ipcHandle['settings:get-ui']({
        sender: {},
      }) as Record<string, unknown>;

      expect(result).toHaveProperty('screenshot');
      expect(result).not.toHaveProperty('wallpaper');
    });

    it('returns only appearance for renderer theme initialization', async () => {
      await loadAndInit();
      const result = ipcHandle['settings:get-appearance']() as Record<
        string,
        unknown
      >;

      expect(result).toHaveProperty('mode');
      expect(result).not.toHaveProperty('wallpaper');
    });

    it('updates config', async () => {
      await loadAndInit();
      const result = ipcHandle['settings:update'](
        { sender: {} },
        { screenshot: { format: 'jpeg' } }
      ) as Record<string, unknown> & { screenshot: { format: string } };
      expect(result.screenshot.format).toBe('jpeg');
      expect(result).not.toHaveProperty('wallpaper');
    });

    it('resets to defaults', async () => {
      await loadAndInit();
      const result = ipcHandle['settings:reset']({ sender: {} });
      expect(result).toBeDefined();
    });

    it('hides cloud credentials from other renderer windows', async () => {
      const settings = await loadAndInit();
      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      settings.updateConfig({
        cloud: {
          ...DEFAULT_SETTINGS.cloud,
          s3: {
            ...DEFAULT_SETTINGS.cloud.s3,
            accessKeyId: 'access-key',
            secretAccessKey: 'secret-key',
          },
          rest: {
            ...DEFAULT_SETTINGS.cloud.rest,
            headers: [{ key: 'Authorization', value: 'Bearer secret' }],
          },
        },
      });
      mockIsSettingsWindowWebContents.mockReturnValue(false);

      const result = ipcHandle['settings:get-ui']({ sender: {} }) as {
        cloud: {
          s3: { accessKeyId: string; secretAccessKey: string };
          rest: { headers: Array<{ key: string; value: string }> };
        };
      };

      expect(result.cloud.s3.accessKeyId).toBe('');
      expect(result.cloud.s3.secretAccessKey).toBe('');
      expect(result.cloud.rest.headers).toEqual([
        { key: 'Authorization', value: '' },
      ]);
    });

    it('does not let other renderer windows replace cloud credentials', async () => {
      const settings = await loadAndInit();
      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      settings.updateConfig({
        cloud: {
          ...DEFAULT_SETTINGS.cloud,
          s3: {
            ...DEFAULT_SETTINGS.cloud.s3,
            secretAccessKey: 'secret-key',
          },
        },
      });
      mockIsSettingsWindowWebContents.mockReturnValue(false);

      ipcHandle['settings:update'](
        { sender: {} },
        {
          cloud: {
            ...DEFAULT_SETTINGS.cloud,
            s3: {
              ...DEFAULT_SETTINGS.cloud.s3,
              secretAccessKey: 'replaced',
            },
          },
        }
      );

      expect(settings.getConfig().cloud.s3.secretAccessKey).toBe('secret-key');
    });

    it('rejects a new cloud secret when OS encryption is unavailable', async () => {
      const settings = await loadAndInit();
      const { DEFAULT_SETTINGS } = await import('@/types/settings');
      mockSafeStorage.isEncryptionAvailable.mockReturnValue(false);
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      const result = ipcHandle['settings:update'](
        { sender: {} },
        {
          cloud: {
            ...DEFAULT_SETTINGS.cloud,
            s3: {
              ...DEFAULT_SETTINGS.cloud.s3,
              secretAccessKey: 'new-secret',
            },
          },
        }
      ) as { cloud: { s3: { secretAccessKey: string } } };

      expect(result.cloud.s3.secretAccessKey).toBe('');
      expect(settings.getConfig().cloud.s3.secretAccessKey).toBe('');
      expect(errorSpy).toHaveBeenCalledWith(
        'Cloud settings update rejected: OS encryption unavailable'
      );
      errorSpy.mockRestore();
    });

    it('does not let generic settings updates replace wallpaper data', async () => {
      const settings = await loadAndInit();

      ipcHandle['settings:update'](
        { sender: {} },
        {
          wallpaper: {
            customBackgrounds: [
              {
                id: 'injected',
                type: 'image',
                data: { imageUrl: 'data:image/png;base64,c2VjcmV0' },
              },
            ],
            presets: [],
            defaultPresetId: null,
          },
        }
      );

      expect(settings.getConfig().wallpaper.customBackgrounds).toEqual([]);
    });

    it('normalizes invalid appearance updates', async () => {
      await loadAndInit();

      const result = ipcHandle['settings:update'](
        { sender: {} },
        { appearance: { mode: 'invalid', theme: 'missing' } }
      ) as { appearance: { mode: string; theme: string } };

      expect(result.appearance).toEqual({ mode: 'dark', theme: 'default' });
    });

    it('confirms and reapplies Windows acrylic for the settings window', async () => {
      const window = {
        isDestroyed: vi.fn(() => false),
        setBackgroundMaterial: vi.fn(),
        setBackgroundColor: vi.fn(),
      };
      mockFromWebContents.mockReturnValue(window);
      await loadAndInit();

      const result = ipcHandle['settings:apply-window-material']({
        sender: {},
      }) as { nativeCapable: boolean };

      const { supportsWindowsAcrylic } = await import('@/main/utils/title-bar');
      const expectAcrylic = result.nativeCapable && supportsWindowsAcrylic();

      expect(window.setBackgroundMaterial.mock.calls).toEqual(
        expectAcrylic ? [['acrylic']] : []
      );
      expect(window.setBackgroundColor.mock.calls).toEqual(
        expectAcrylic ? [['#00000000']] : []
      );
    });
  });

  describe('app:getVersion', () => {
    it('returns app version', async () => {
      await loadAndInit();
      const result = ipcHandle['app:getVersion']();
      expect(typeof result).toBe('string');
    });
  });

  describe('editor preferences', () => {
    it('getPreferences returns editor settings', async () => {
      await loadAndInit();
      const result = ipcHandle['editor:getPreferences']();
      expect(result).toBeDefined();
    });

    it('updatePreferences merges editor settings', async () => {
      await loadAndInit();
      const result = ipcHandle['editor:updatePreferences'](
        {},
        { showCursor: false }
      );
      expect(result).toBeDefined();
    });
  });

  describe('wallpaper handlers', () => {
    it('resolves a project wallpaper after its folder is renamed', async () => {
      mockExistsSync.mockReturnValue(false);
      mockReadFileSync.mockReturnValue(
        Buffer.from([255, 216, 255, 224, 0, 16, 74, 70, 73, 70])
      );
      const { localizeWallpaperImage, resolveLocalizedWallpaperImage } =
        await import('@/main/settings/wallpaper-assets');
      const oldDirectory = path.resolve('recordings', 'Old.poratake');
      const newDirectory = path.resolve('recordings', 'New.poratake');

      const localized = localizeWallpaperImage(
        pathToFileURL(path.resolve('config', 'wallpaper.jpg')).href,
        oldDirectory
      );
      const resolved = resolveLocalizedWallpaperImage(localized, newDirectory);

      expect(localized).toMatch(/^\.wallpaper-asset-.*\.jpg$/);
      expect(fileURLToPath(resolved)).toBe(path.join(newDirectory, localized));
    });

    it('retains prior project wallpaper assets for editor undo', async () => {
      mockExistsSync.mockReturnValue(false);
      mockReadFileSync.mockReturnValue(
        Buffer.from([255, 216, 255, 224, 0, 16, 74, 70, 73, 70])
      );
      mockReaddirSync.mockReturnValue(['.wallpaper-asset-prior.jpg']);
      const { localizeWallpaperImage } =
        await import('@/main/settings/wallpaper-assets');

      localizeWallpaperImage(
        pathToFileURL(path.resolve('config', 'wallpaper.jpg')).href,
        path.resolve('recordings', 'Take.poratake')
      );

      expect(mockUnlinkSync).not.toHaveBeenCalled();
    });

    it('getSettings returns wallpaper config', async () => {
      await loadAndInit();
      const result = ipcHandle['wallpaper:getSettings']();
      expect(result).toBeDefined();
    });

    it('addBackground appends background', async () => {
      await loadAndInit();
      const bg = { id: 'bg1', type: 'color' as const, data: { color: '#fff' } };
      const result = ipcHandle['wallpaper:addBackground']({}, bg) as unknown[];
      expect(result.length).toBeGreaterThan(0);
    });

    it('updateBackground replaces matching id', async () => {
      await loadAndInit();
      const bg = { id: 'bg1', type: 'color' as const, data: { color: '#fff' } };
      ipcHandle['wallpaper:addBackground']({}, bg);
      const updated = {
        id: 'bg1',
        type: 'color' as const,
        data: { color: '#000' },
      };
      const result = ipcHandle['wallpaper:updateBackground'](
        {},
        updated
      ) as Array<{
        id: string;
        data: { color: string };
      }>;
      expect(result.find(b => b.id === 'bg1')?.data.color).toBe('#000');
    });

    it('updateBackground ignores unknown ids', async () => {
      await loadAndInit();
      const bg = {
        id: 'nope',
        type: 'color' as const,
        data: { color: '#000' },
      };
      const result = ipcHandle['wallpaper:updateBackground'](
        {},
        bg
      ) as unknown[];
      expect(result).toBeDefined();
    });

    it('deleteBackground removes by id', async () => {
      await loadAndInit();
      const bg = { id: 'bg2', type: 'color' as const, data: { color: '#fff' } };
      ipcHandle['wallpaper:addBackground']({}, bg);
      const result = ipcHandle['wallpaper:deleteBackground'](
        {},
        'bg2'
      ) as Array<{
        id: string;
      }>;
      expect(result.find(b => b.id === 'bg2')).toBeUndefined();
    });

    it('addPreset appends preset', async () => {
      await loadAndInit();
      const preset = { id: 'p1', name: 'My', backgroundId: 'bg1' };
      const result = ipcHandle['wallpaper:addPreset']({}, preset) as unknown[];
      expect(result.length).toBeGreaterThan(0);
    });

    it('updatePreset modifies matching preset', async () => {
      await loadAndInit();
      const preset = { id: 'p2', name: 'A', backgroundId: 'bg1' };
      ipcHandle['wallpaper:addPreset']({}, preset);
      const updated = { id: 'p2', name: 'B', backgroundId: 'bg1' };
      const result = ipcHandle['wallpaper:updatePreset']({}, updated) as Array<{
        id: string;
        name: string;
      }>;
      expect(result.find(p => p.id === 'p2')?.name).toBe('B');
    });

    it('updatePreset ignores unknown ids', async () => {
      await loadAndInit();
      const result = ipcHandle['wallpaper:updatePreset'](
        {},
        { id: 'missing', name: 'X', backgroundId: 'bg' }
      );
      expect(result).toBeDefined();
    });

    it('deletePreset removes by id', async () => {
      await loadAndInit();
      const preset = { id: 'p3', name: 'A', backgroundId: 'bg1' };
      ipcHandle['wallpaper:addPreset']({}, preset);
      const result = ipcHandle['wallpaper:deletePreset']({}, 'p3') as Array<{
        id: string;
      }>;
      expect(result.find(p => p.id === 'p3')).toBeUndefined();
    });

    it('setDefaultPreset stores an existing preset id', async () => {
      await loadAndInit();
      ipcHandle['wallpaper:addPreset']({}, { id: 'p4', name: 'A' });
      expect(ipcHandle['wallpaper:setDefaultPreset']({}, 'p4')).toBe('p4');
      expect(ipcHandle['wallpaper:getSettings']()).toMatchObject({
        defaultPresetId: 'p4',
      });
    });

    it('setDefaultPreset rejects unknown ids and clears on null', async () => {
      await loadAndInit();
      ipcHandle['wallpaper:addPreset']({}, { id: 'p5', name: 'A' });
      ipcHandle['wallpaper:setDefaultPreset']({}, 'p5');

      expect(ipcHandle['wallpaper:setDefaultPreset']({}, 'missing')).toBeNull();
      expect(ipcHandle['wallpaper:setDefaultPreset']({}, null)).toBeNull();
    });

    it('deletePreset clears the default when it is deleted', async () => {
      await loadAndInit();
      ipcHandle['wallpaper:addPreset']({}, { id: 'p6', name: 'A' });
      ipcHandle['wallpaper:addPreset']({}, { id: 'p7', name: 'B' });
      ipcHandle['wallpaper:setDefaultPreset']({}, 'p6');

      ipcHandle['wallpaper:deletePreset']({}, 'p7');
      expect(ipcHandle['wallpaper:getSettings']()).toMatchObject({
        defaultPresetId: 'p6',
      });

      ipcHandle['wallpaper:deletePreset']({}, 'p6');
      expect(ipcHandle['wallpaper:getSettings']()).toMatchObject({
        defaultPresetId: null,
      });
    });

    it('selectImage returns null on cancel', async () => {
      mockShowOpenDialog.mockResolvedValue({ canceled: true, filePaths: [] });
      await loadAndInit();
      const result = await ipcHandle['wallpaper:selectImage']();
      expect(result).toBeNull();
    });

    it('selectImage returns the selected file URL on success', async () => {
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/p/img.png'],
      });
      await loadAndInit();
      const result = (await ipcHandle['wallpaper:selectImage']()) as string;
      expect(result).toMatch(/^file:/);
      expect(result).toContain('/p/img.png');
    });

    it('selectImage returns SVG, JPEG, and WebP file URLs', async () => {
      for (const ext of ['.svg', '.jpg', '.webp']) {
        mockShowOpenDialog.mockResolvedValue({
          canceled: false,
          filePaths: [`/p/img${ext}`],
        });
        await loadAndInit();
        const result = (await ipcHandle['wallpaper:selectImage']()) as string;
        expect(result).toMatch(/^file:/);
        expect(result).toContain(`/p/img${ext}`);
      }
    });

    it('selectImage returns null on error', async () => {
      mockShowOpenDialog.mockRejectedValue(new Error('boom'));
      await loadAndInit();
      const result = await ipcHandle['wallpaper:selectImage']();
      expect(result).toBeNull();
    });

    it('getDesktopWallpaper returns null when daemon returns null', async () => {
      mockDaemonCall.mockResolvedValue(null);
      await loadAndInit();
      const result = await ipcHandle['wallpaper:getDesktopWallpaper']();
      expect(result).toBeNull();
    });

    it('getDesktopWallpaper returns a data response', async () => {
      mockDaemonCall.mockResolvedValue({
        type: 'data',
        value: 'data:image/png;base64,abc',
      });
      await loadAndInit();
      const result = (await ipcHandle[
        'wallpaper:getDesktopWallpaper'
      ]()) as string;
      expect(result).toBe('data:image/png;base64,abc');
    });

    it('getDesktopWallpaper returns a file URL for a wallpaper path', async () => {
      const wallpaperPath = '/p/wallpaper.jpg';
      mockDaemonCall.mockResolvedValue({
        type: 'path',
        value: wallpaperPath,
      });
      mockExistsSync.mockReturnValue(true);
      await loadAndInit();
      const result = (await ipcHandle[
        'wallpaper:getDesktopWallpaper'
      ]()) as string;
      expect(result).toMatch(/^file:/);
      expect(result).toContain('/p/wallpaper.jpg');
      expect(mockReadFileSync).not.toHaveBeenCalledWith(wallpaperPath);
    });

    it('persists an extensionless Windows desktop wallpaper by file signature', async () => {
      mockExistsSync.mockReturnValue(false);
      await loadAndInit();
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(
        Buffer.from([255, 216, 255, 224, 0, 16, 74, 70, 73, 70])
      );

      const result = ipcHandle['wallpaper:addPreset'](
        {},
        {
          id: 'desktop',
          name: 'Desktop',
          gradient: null,
          backgroundImage: 'file:///E:/p/TranscodedWallpaper',
          padding: 0,
          corners: 0,
          shadow: 0,
        }
      ) as Array<{ backgroundImage?: string }>;

      expect(result[0].backgroundImage).toMatch(/\/wallpapers\/.*\.jpg$/);
    });

    it('keeps a managed wallpaper asset until the next startup', async () => {
      mockExistsSync.mockReturnValue(false);
      await loadAndInit();
      mockExistsSync.mockReturnValue(true);
      mockReadFileSync.mockReturnValue(
        Buffer.from([255, 216, 255, 224, 0, 16, 74, 70, 73, 70])
      );
      const backgrounds = ipcHandle['wallpaper:addBackground'](
        {},
        {
          id: 'managed',
          type: 'image',
          data: { imageUrl: 'file:///E:/p/wallpaper.jpg' },
        }
      ) as Array<{ data: { imageUrl: string } }>;
      const assetName = decodeURIComponent(
        new URL(backgrounds[0].data.imageUrl).pathname.split('/').at(-1) ?? ''
      );
      mockReaddirSync.mockReturnValue([assetName]);

      ipcHandle['wallpaper:deleteBackground']({}, 'managed');

      expect(mockUnlinkSync).not.toHaveBeenCalled();
    });

    it('getDesktopWallpaper returns null when path file missing', async () => {
      mockDaemonCall.mockResolvedValue({
        type: 'path',
        value: '/p/missing.png',
      });
      mockExistsSync.mockReturnValue(false);
      await loadAndInit();
      const result = await ipcHandle['wallpaper:getDesktopWallpaper']();
      expect(result).toBeNull();
    });

    it('getDesktopWallpaper returns null on daemon error', async () => {
      mockDaemonCall.mockRejectedValue(new Error('boom'));
      await loadAndInit();
      const result = await ipcHandle['wallpaper:getDesktopWallpaper']();
      expect(result).toBeNull();
    });
  });

  describe('storage handlers', () => {
    it('selectPath returns null on cancel', async () => {
      mockShowOpenDialog.mockResolvedValue({ canceled: true, filePaths: [] });
      await loadAndInit();
      const result = await ipcHandle['storage:selectPath']({}, 'screenshots');
      expect(result).toBeNull();
    });

    it('selectPath returns path on success', async () => {
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/custom/screenshots'],
      });
      await loadAndInit();
      const result = (await ipcHandle['storage:selectPath'](
        {},
        'recordings'
      )) as { path?: string; error?: string } | null;
      expect(result?.path).toBe('/custom/screenshots');
    });

    it('selectPath returns error when not a directory', async () => {
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/p'],
      });
      mockStatSync.mockReturnValue({ isDirectory: () => false });
      await loadAndInit();
      const result = (await ipcHandle['storage:selectPath'](
        {},
        'screenshots'
      )) as {
        error?: string;
      };
      expect(result.error).toBeDefined();
    });

    it('validatePattern delegates to filename generator', async () => {
      await loadAndInit();
      const result = ipcHandle['storage:validatePattern']({}, 'Pattern');
      expect(result).toBeDefined();
    });

    it('previewFilename generates filename', async () => {
      await loadAndInit();
      const result = ipcHandle['storage:previewFilename'](
        {},
        '{date}',
        'Screenshot'
      );
      expect(typeof result).toBe('string');
    });

    it('getTokens returns available tokens', async () => {
      await loadAndInit();
      const result = ipcHandle['storage:getTokens']() as unknown[];
      expect(Array.isArray(result)).toBe(true);
    });

    it('getDefaultPaths returns default paths', async () => {
      await loadAndInit();
      const result = ipcHandle['storage:getDefaultPaths']() as {
        screenshots: string;
        recordings: string;
      };
      expect(result.screenshots).toContain('Poratake');
      expect(result.recordings).toContain('Poratake');
    });

    it('getDefaultPaths repairs empty paths with Poratake defaults', async () => {
      mockReadFileSync.mockReturnValue(
        JSON.stringify({ storage: { screenshotsPath: '', recordingsPath: '' } })
      );
      await loadAndInit();
      const result = ipcHandle['storage:getDefaultPaths']() as {
        screenshots: string;
        recordings: string;
      };
      expect(result.screenshots).toContain('Poratake');
      expect(result.recordings).toContain('Poratake');
    });
  });

  describe('cursor:selectImage', () => {
    it('returns null on cancel', async () => {
      mockShowOpenDialog.mockResolvedValue({ canceled: true, filePaths: [] });
      await loadAndInit();
      const result = await ipcHandle['cursor:selectImage']();
      expect(result).toBeNull();
    });

    it('returns data URL on success', async () => {
      mockShowOpenDialog.mockResolvedValue({
        canceled: false,
        filePaths: ['/p/cursor.gif'],
      });
      await loadAndInit();
      const result = (await ipcHandle['cursor:selectImage']()) as string;
      expect(result).toMatch(/^data:image\/gif;base64,/);
    });

    it('returns null on error', async () => {
      mockShowOpenDialog.mockRejectedValue(new Error('boom'));
      await loadAndInit();
      const result = await ipcHandle['cursor:selectImage']();
      expect(result).toBeNull();
    });
  });
});
