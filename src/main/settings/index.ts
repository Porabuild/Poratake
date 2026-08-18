import { app, BrowserWindow, dialog, ipcMain } from 'electron';
import fs from 'fs';
import path from 'path';
import { pathToFileURL } from 'url';
import { daemon } from '@/main/daemon';
import { isSettingsWindowWebContents } from '@/main/settings/window';
import {
  applyTitleBarAppearance,
  supportsNativeWindowMaterial,
  supportsWindowsAcrylic,
} from '@/main/utils/title-bar';
import type {
  CloudConfig,
  CustomBackground,
  CustomGradient,
  AppearanceConfig,
  AllInOneConfig,
  RecordingSettings,
  SettingsConfig,
  SettingsUiConfig,
  WallpaperPreset,
} from '@/types/settings.ts';
import { APP_THEME_PRESETS } from '@/types/theme';
import {
  DEFAULT_CLOUD_CONFIG,
  DEFAULT_REST_PROVIDER_CONFIG,
  DEFAULT_S3_PROVIDER_CONFIG,
  DEFAULT_SETTINGS,
  DEFAULT_STORAGE_CONFIG,
  DEFAULT_ALL_IN_ONE_CONFIG,
  DEFAULT_PREVIEW_CONFIG,
  DEFAULT_SAVE_LOCATIONS_CONFIG,
} from '@/types/settings.ts';
import {
  generateFilename,
  validateNamingPattern,
  getAvailableTokens,
  type CaptureType,
} from '@/main/utils/filename-generator';
import { getConfigDir, getConfigFilePath } from '@/main/utils/paths.ts';
import { getAppVersion } from '@/main/utils/env.ts';

export { createOrShowSettingsWindow } from './window';

const CONFIG_DIR = getConfigDir();
const CONFIG_FILE = getConfigFilePath();
const CONFIG_WRITE_DEBOUNCE_MS = 150;

let currentConfig: SettingsConfig = { ...DEFAULT_SETTINGS };
let configLoaded = false;
let configWriteTimer: NodeJS.Timeout | null = null;
let configWritePending = false;
let configWriteInFlight = false;
let configWriteGeneration = 0;
let configWriteSequence = 0;
let configWriteQueue: Promise<void> = Promise.resolve();
const configUpdateListeners = new Set<
  (updates: Partial<SettingsConfig>) => void
>();

export function onConfigUpdated(
  listener: (updates: Partial<SettingsConfig>) => void
): () => void {
  configUpdateListeners.add(listener);
  return () => configUpdateListeners.delete(listener);
}

const APPEARANCE_MODES = new Set(['system', 'light', 'dark']);
const APP_THEME_IDS = new Set(APP_THEME_PRESETS.map(theme => theme.id));

function migrateAppearanceConfig(
  savedAppearance?: Partial<AppearanceConfig>
): AppearanceConfig {
  return {
    mode: APPEARANCE_MODES.has(savedAppearance?.mode ?? '')
      ? (savedAppearance?.mode ?? DEFAULT_SETTINGS.appearance.mode)
      : DEFAULT_SETTINGS.appearance.mode,
    theme: APP_THEME_IDS.has(savedAppearance?.theme ?? '')
      ? (savedAppearance?.theme ?? DEFAULT_SETTINGS.appearance.theme)
      : DEFAULT_SETTINGS.appearance.theme,
  };
}

type SavedAllInOneConfig = Partial<
  Omit<AllInOneConfig, 'lastTargets' | 'recording'>
> & {
  lastTargets?: Partial<AllInOneConfig['lastTargets']>;
  recording?: Partial<AllInOneConfig['recording']>;
};

function migrateAllInOneConfig(
  savedAllInOne?: SavedAllInOneConfig,
  savedRecording?: Partial<RecordingSettings>
): AllInOneConfig {
  return {
    ...DEFAULT_ALL_IN_ONE_CONFIG,
    ...savedAllInOne,
    lastTargets: {
      ...DEFAULT_ALL_IN_ONE_CONFIG.lastTargets,
      ...savedAllInOne?.lastTargets,
    },
    recording: {
      systemAudio:
        savedAllInOne?.recording?.systemAudio ??
        savedRecording?.systemAudio ??
        DEFAULT_ALL_IN_ONE_CONFIG.recording.systemAudio,
      micEnabled:
        savedAllInOne?.recording?.micEnabled ??
        savedRecording?.micEnabled ??
        DEFAULT_ALL_IN_ONE_CONFIG.recording.micEnabled,
      cameraEnabled:
        savedAllInOne?.recording?.cameraEnabled ??
        savedRecording?.camera?.enabled ??
        DEFAULT_ALL_IN_ONE_CONFIG.recording.cameraEnabled,
    },
  };
}

function getSettingsUiConfig(
  config: SettingsConfig,
  includeCloudCredentials: boolean
): SettingsUiConfig {
  const settings = {
    ...config,
    cloud: includeCloudCredentials
      ? config.cloud
      : {
          ...config.cloud,
          s3: {
            ...config.cloud.s3,
            accessKeyId: '',
            secretAccessKey: '',
          },
          rest: {
            ...config.cloud.rest,
            headers: config.cloud.rest.headers.map(header => ({
              ...header,
              value: '',
            })),
          },
        },
  };
  Reflect.deleteProperty(settings, 'wallpaper');
  return settings;
}

function migrateStorageConfig(
  savedStorage?: Partial<SettingsConfig['storage']>
): SettingsConfig['storage'] {
  const storage = { ...DEFAULT_STORAGE_CONFIG, ...savedStorage };

  return {
    ...storage,
    screenshotsPath:
      storage.screenshotsPath || path.join(app.getPath('pictures'), 'Poratake'),
    recordingsPath:
      storage.recordingsPath || path.join(app.getPath('videos'), 'Poratake'),
  };
}

function migrateWallpaperConfig(
  savedWallpaper?: SettingsConfig['wallpaper']
): SettingsConfig['wallpaper'] {
  const base = {
    ...DEFAULT_SETTINGS.wallpaper,
    ...savedWallpaper,
  };

  const presets = base.presets ?? [];
  const defaultPresetId = presets.some(p => p.id === base.defaultPresetId)
    ? base.defaultPresetId
    : null;

  if (base.customBackgrounds && base.customBackgrounds.length > 0) {
    return {
      customBackgrounds: base.customBackgrounds,
      presets,
      defaultPresetId,
    };
  }

  const legacyGradients =
    (savedWallpaper as { customGradients?: CustomGradient[] })
      ?.customGradients ?? [];
  const migratedBackgrounds: CustomBackground[] = legacyGradients
    .filter(g => g.colors && g.angle !== undefined)
    .map(g => ({
      id: g.id,
      type: 'gradient' as const,
      data: {
        colors: g.colors!,
        angle: g.angle!,
      },
    }));

  return {
    customBackgrounds: migratedBackgrounds,
    presets,
    defaultPresetId,
  };
}

export function migrateCloudConfig(savedCloud: unknown): CloudConfig {
  if (!savedCloud || typeof savedCloud !== 'object') {
    return { ...DEFAULT_CLOUD_CONFIG };
  }

  const raw = savedCloud as Record<string, unknown>;
  const hasNestedS3 = raw.s3 && typeof raw.s3 === 'object';
  const hasLegacyFlatS3 =
    typeof raw.endpoint === 'string' ||
    typeof raw.bucket === 'string' ||
    typeof raw.accessKeyId === 'string';

  const s3Source = hasNestedS3
    ? (raw.s3 as Record<string, unknown>)
    : hasLegacyFlatS3
      ? raw
      : {};

  const restSource =
    raw.rest && typeof raw.rest === 'object'
      ? (raw.rest as Record<string, unknown>)
      : {};
  const hasConfiguredS3 = [
    s3Source.endpoint,
    s3Source.bucket,
    s3Source.accessKeyId,
    s3Source.secretAccessKey,
  ].some(value => typeof value === 'string' && value.length > 0);
  const hasConfiguredRest =
    (typeof restSource.url === 'string' && restSource.url.length > 0) ||
    (Array.isArray(restSource.headers) && restSource.headers.length > 0) ||
    (typeof restSource.responseUrlPath === 'string' &&
      restSource.responseUrlPath.length > 0);

  let activeProvider = DEFAULT_CLOUD_CONFIG.activeProvider;

  if (raw.activeProvider === 'rest' && hasConfiguredRest) {
    activeProvider = 'rest';
  }

  if ((raw.activeProvider === 's3' || hasLegacyFlatS3) && hasConfiguredS3) {
    activeProvider = 's3';
  }

  const enabled =
    raw.activeProvider === 'capty'
      ? false
      : typeof raw.enabled === 'boolean'
        ? raw.enabled
        : hasLegacyFlatS3 && hasConfiguredS3
          ? true
          : DEFAULT_CLOUD_CONFIG.enabled;

  return {
    enabled,
    activeProvider,
    s3: { ...DEFAULT_S3_PROVIDER_CONFIG, ...s3Source },
    rest: {
      ...DEFAULT_REST_PROVIDER_CONFIG,
      ...restSource,
    },
  };
}

function ensureConfigDir() {
  if (!fs.existsSync(CONFIG_DIR)) {
    fs.mkdirSync(CONFIG_DIR, { recursive: true });
  }
}

export function loadConfig(): SettingsConfig {
  if (configLoaded) {
    return currentConfig;
  }

  try {
    ensureConfigDir();
    if (fs.existsSync(CONFIG_FILE)) {
      const fileContent = fs.readFileSync(CONFIG_FILE, 'utf-8');
      const savedConfig = JSON.parse(fileContent);
      currentConfig = {
        appearance: migrateAppearanceConfig(savedConfig.appearance),
        general: { ...DEFAULT_SETTINGS.general, ...savedConfig.general },
        screenshot: {
          ...DEFAULT_SETTINGS.screenshot,
          ...savedConfig.screenshot,
        },
        shortcuts: {
          screenshot: {
            ...DEFAULT_SETTINGS.shortcuts.screenshot,
            ...savedConfig.shortcuts?.screenshot,
          },
          captureText:
            savedConfig.shortcuts?.captureText ??
            DEFAULT_SETTINGS.shortcuts.captureText,
          scanQRCode:
            savedConfig.shortcuts?.scanQRCode ??
            DEFAULT_SETTINGS.shortcuts.scanQRCode,
          timerCapture:
            savedConfig.shortcuts?.timerCapture ??
            DEFAULT_SETTINGS.shortcuts.timerCapture,
          scrollCapture:
            savedConfig.shortcuts?.scrollCapture ??
            DEFAULT_SETTINGS.shortcuts.scrollCapture,
          recording: {
            ...DEFAULT_SETTINGS.shortcuts.recording,
            ...savedConfig.shortcuts?.recording,
          },
          history:
            savedConfig.shortcuts?.history ??
            DEFAULT_SETTINGS.shortcuts.history,
          allInOne:
            savedConfig.shortcuts?.allInOne ??
            DEFAULT_SETTINGS.shortcuts.allInOne,
          openInEditor:
            savedConfig.shortcuts?.openInEditor ??
            DEFAULT_SETTINGS.shortcuts.openInEditor,
          clipboardInEditor:
            savedConfig.shortcuts?.clipboardInEditor ??
            DEFAULT_SETTINGS.shortcuts.clipboardInEditor,
          editor: {
            ...DEFAULT_SETTINGS.shortcuts.editor,
            ...savedConfig.shortcuts?.editor,
          },
          editorActions: {
            ...DEFAULT_SETTINGS.shortcuts.editorActions,
            ...savedConfig.shortcuts?.editorActions,
          },
          videoEditorSidebar: {
            ...DEFAULT_SETTINGS.shortcuts.videoEditorSidebar,
            ...savedConfig.shortcuts?.videoEditorSidebar,
          },
        },
        editor: { ...DEFAULT_SETTINGS.editor, ...savedConfig.editor },
        wallpaper: migrateWallpaperConfig(savedConfig.wallpaper),
        history: { ...DEFAULT_SETTINGS.history, ...savedConfig.history },
        onboarding: {
          ...DEFAULT_SETTINGS.onboarding,
          ...savedConfig.onboarding,
        },
        cloud: migrateCloudConfig(savedConfig.cloud),
        recording: { ...DEFAULT_SETTINGS.recording, ...savedConfig.recording },
        storage: migrateStorageConfig(savedConfig.storage),
        saveLocations: {
          ...DEFAULT_SAVE_LOCATIONS_CONFIG,
          ...savedConfig.saveLocations,
        },
        preview: { ...DEFAULT_PREVIEW_CONFIG, ...savedConfig.preview },
        allInOne: migrateAllInOneConfig(
          savedConfig.allInOne,
          savedConfig.recording
        ),
        scrollCapture: {
          ...DEFAULT_SETTINGS.scrollCapture,
          ...savedConfig.scrollCapture,
        },
      };
    } else {
      currentConfig = { ...DEFAULT_SETTINGS };
      saveConfig(currentConfig);
    }
    configLoaded = true;
  } catch (error) {
    console.error('Failed to load config:', error);
    currentConfig = { ...DEFAULT_SETTINGS };
    configLoaded = true;
  }
  return currentConfig;
}

function writeConfigFileSync(): void {
  configWriteGeneration += 1;
  const tempFile = `${CONFIG_FILE}.flush.tmp`;
  fs.writeFileSync(tempFile, JSON.stringify(currentConfig, null, 2), 'utf-8');
  fs.renameSync(tempFile, CONFIG_FILE);
}

async function writeConfigFile(): Promise<void> {
  configWriteSequence += 1;
  configWriteGeneration += 1;
  const tempFile = `${CONFIG_FILE}.${process.pid}.${configWriteSequence}.tmp`;
  const generation = configWriteGeneration;
  configWriteInFlight = true;

  try {
    ensureConfigDir();
    await fs.promises.writeFile(
      tempFile,
      JSON.stringify(currentConfig, null, 2),
      'utf-8'
    );
    if (generation !== configWriteGeneration) {
      await fs.promises.unlink(tempFile).catch(() => {});
      return;
    }
    await fs.promises.rename(tempFile, CONFIG_FILE);
  } finally {
    configWriteInFlight = false;
  }
}

function flushConfigFile(): void {
  if (configWriteTimer) {
    clearTimeout(configWriteTimer);
    configWriteTimer = null;
  }

  if (!configWritePending && !configWriteInFlight) {
    return;
  }
  configWritePending = false;

  try {
    ensureConfigDir();
    writeConfigFileSync();
  } catch (error) {
    console.error('Failed to save config:', error);
  }
}

export function saveConfig(config: SettingsConfig): void {
  currentConfig = config;
  configWritePending = true;

  if (configWriteTimer) {
    clearTimeout(configWriteTimer);
  }
  configWriteTimer = setTimeout(() => {
    configWriteTimer = null;
    configWriteQueue = configWriteQueue
      .then(async () => {
        if (!configWritePending) {
          return;
        }
        configWritePending = false;
        await writeConfigFile();
      })
      .catch(error => {
        console.error('Failed to save config:', error);
      });
  }, CONFIG_WRITE_DEBOUNCE_MS);
}

export function getConfig(): SettingsConfig {
  if (!configLoaded) {
    loadConfig();
  }
  return currentConfig;
}

export function updateConfig(updates: Partial<SettingsConfig>): SettingsConfig {
  if (!configLoaded) {
    loadConfig();
  }

  currentConfig = {
    ...currentConfig,
    ...updates,
    appearance: migrateAppearanceConfig({
      ...currentConfig.appearance,
      ...updates.appearance,
    }),
    general: { ...currentConfig.general, ...updates.general },
    screenshot: { ...currentConfig.screenshot, ...updates.screenshot },
    shortcuts: {
      screenshot: {
        ...currentConfig.shortcuts.screenshot,
        ...updates.shortcuts?.screenshot,
      },
      captureText:
        updates.shortcuts?.captureText ?? currentConfig.shortcuts.captureText,
      scanQRCode:
        updates.shortcuts?.scanQRCode ?? currentConfig.shortcuts.scanQRCode,
      timerCapture:
        updates.shortcuts?.timerCapture ?? currentConfig.shortcuts.timerCapture,
      scrollCapture:
        updates.shortcuts?.scrollCapture ??
        currentConfig.shortcuts.scrollCapture,
      recording: {
        ...currentConfig.shortcuts.recording,
        ...updates.shortcuts?.recording,
      },
      history: updates.shortcuts?.history ?? currentConfig.shortcuts.history,
      allInOne: updates.shortcuts?.allInOne ?? currentConfig.shortcuts.allInOne,
      openInEditor:
        updates.shortcuts?.openInEditor ?? currentConfig.shortcuts.openInEditor,
      clipboardInEditor:
        updates.shortcuts?.clipboardInEditor ??
        currentConfig.shortcuts.clipboardInEditor,
      editor: {
        ...currentConfig.shortcuts.editor,
        ...updates.shortcuts?.editor,
      },
      editorActions: {
        ...currentConfig.shortcuts.editorActions,
        ...updates.shortcuts?.editorActions,
      },
      videoEditorSidebar: {
        ...currentConfig.shortcuts.videoEditorSidebar,
        ...updates.shortcuts?.videoEditorSidebar,
      },
    },
    editor: { ...currentConfig.editor, ...updates.editor },
    wallpaper: { ...currentConfig.wallpaper, ...updates.wallpaper },
    history: { ...currentConfig.history, ...updates.history },
    onboarding: { ...currentConfig.onboarding, ...updates.onboarding },
    cloud: {
      ...currentConfig.cloud,
      ...updates.cloud,
      s3: { ...currentConfig.cloud.s3, ...updates.cloud?.s3 },
      rest: {
        ...currentConfig.cloud.rest,
        ...updates.cloud?.rest,
        headers:
          updates.cloud?.rest?.headers ?? currentConfig.cloud.rest.headers,
      },
    },
    recording: { ...currentConfig.recording, ...updates.recording },
    storage: { ...currentConfig.storage, ...updates.storage },
    saveLocations: { ...currentConfig.saveLocations, ...updates.saveLocations },
    preview: { ...currentConfig.preview, ...updates.preview },
    allInOne: { ...currentConfig.allInOne, ...updates.allInOne },
    scrollCapture: { ...currentConfig.scrollCapture, ...updates.scrollCapture },
  };

  if (updates.general?.startOnLogin !== undefined) {
    try {
      app.setLoginItemSettings({
        openAtLogin: updates.general.startOnLogin,
      });
    } catch (error) {
      console.warn('Failed to set login item:', error);
    }
  }

  saveConfig(currentConfig);
  configUpdateListeners.forEach(listener => listener(updates));
  return currentConfig;
}

export function needsOnboarding(): boolean {
  const config = getConfig();
  return !config.onboarding.completed && !config.onboarding.skipped;
}

export function markOnboardingCompleted(): void {
  updateConfig({ onboarding: { completed: true, skipped: false } });
}

export function markOnboardingSkipped(): void {
  updateConfig({ onboarding: { completed: false, skipped: true } });
}

function applyLoginItemSetting() {
  const config = getConfig();
  try {
    app.setLoginItemSettings({
      openAtLogin: config.general.startOnLogin,
    });
  } catch (error) {
    console.warn('Failed to apply login item setting:', error);
  }
}

export function init() {
  const config = loadConfig();
  applyTitleBarAppearance(config.appearance);

  app.on('will-quit', flushConfigFile);

  applyLoginItemSetting();

  ipcMain.handle('settings:get-ui', event => {
    return getSettingsUiConfig(
      getConfig(),
      isSettingsWindowWebContents(event.sender)
    );
  });

  ipcMain.handle('settings:get-appearance', () => {
    return getConfig().appearance;
  });

  ipcMain.handle('settings:apply-window-material', event => {
    const nativeCapable = supportsNativeWindowMaterial();
    const window = BrowserWindow.fromWebContents(event.sender);
    if (!nativeCapable || !window || window.isDestroyed()) {
      return { nativeCapable: false };
    }

    if (supportsWindowsAcrylic()) {
      window.setBackgroundMaterial('acrylic');
      window.setBackgroundColor('#00000000');
    }
    return { nativeCapable: true };
  });

  ipcMain.handle(
    'settings:update',
    (event, updates: Partial<SettingsConfig>) => {
      const isSettingsWindow = isSettingsWindowWebContents(event.sender);
      const allowedUpdates = { ...updates };
      if (!isSettingsWindow) {
        Reflect.deleteProperty(allowedUpdates, 'cloud');
      }
      const updatedConfig = updateConfig(allowedUpdates);

      if (allowedUpdates.appearance) {
        applyTitleBarAppearance(updatedConfig.appearance);
      }

      if (allowedUpdates.appearance) {
        BrowserWindow.getAllWindows().forEach(win => {
          win.webContents.send(
            'settings:appearance-updated',
            updatedConfig.appearance
          );
        });
      }

      if (allowedUpdates.screenshot) {
        BrowserWindow.getAllWindows().forEach(win => {
          win.webContents.send('screenshot-settings:updated', {
            closeOnCopy: updatedConfig.screenshot.closeOnCopy,
            closeOnSave: updatedConfig.screenshot.closeOnSave,
            format: updatedConfig.screenshot.format,
          });
        });
      }

      return getSettingsUiConfig(updatedConfig, isSettingsWindow);
    }
  );

  ipcMain.handle('settings:reset', event => {
    if (!isSettingsWindowWebContents(event.sender)) {
      return getSettingsUiConfig(currentConfig, false);
    }

    currentConfig = { ...DEFAULT_SETTINGS };
    saveConfig(currentConfig);
    configUpdateListeners.forEach(listener => listener(currentConfig));
    applyTitleBarAppearance(currentConfig.appearance);
    applyLoginItemSetting();
    BrowserWindow.getAllWindows().forEach(win => {
      win.webContents.send(
        'settings:appearance-updated',
        currentConfig.appearance
      );
    });
    return currentConfig;
  });

  ipcMain.handle('app:getVersion', () => {
    return getAppVersion();
  });

  ipcMain.handle('editor:getPreferences', () => {
    return getConfig().editor;
  });

  ipcMain.handle(
    'editor:updatePreferences',
    (_event, updates: Partial<SettingsConfig['editor']>) => {
      return updateConfig({ editor: { ...currentConfig.editor, ...updates } })
        .editor;
    }
  );

  ipcMain.handle('wallpaper:getSettings', () => {
    return getConfig().wallpaper;
  });

  ipcMain.handle(
    'wallpaper:addBackground',
    (_event, background: CustomBackground) => {
      const wallpaper = currentConfig.wallpaper;
      wallpaper.customBackgrounds.push(background);
      updateConfig({ wallpaper });
      return wallpaper.customBackgrounds;
    }
  );

  ipcMain.handle(
    'wallpaper:updateBackground',
    (_event, background: CustomBackground) => {
      const wallpaper = currentConfig.wallpaper;
      const index = wallpaper.customBackgrounds.findIndex(
        b => b.id === background.id
      );
      if (index !== -1) {
        wallpaper.customBackgrounds[index] = background;
        updateConfig({ wallpaper });
      }
      return wallpaper.customBackgrounds;
    }
  );

  ipcMain.handle('wallpaper:deleteBackground', (_event, id: string) => {
    const wallpaper = currentConfig.wallpaper;
    wallpaper.customBackgrounds = wallpaper.customBackgrounds.filter(
      b => b.id !== id
    );
    updateConfig({ wallpaper });
    return wallpaper.customBackgrounds;
  });

  ipcMain.handle('wallpaper:addPreset', (_event, preset: WallpaperPreset) => {
    const wallpaper = currentConfig.wallpaper;
    wallpaper.presets.push(preset);
    updateConfig({ wallpaper });
    return wallpaper.presets;
  });

  ipcMain.handle(
    'wallpaper:updatePreset',
    (_event, preset: WallpaperPreset) => {
      const wallpaper = currentConfig.wallpaper;
      const index = wallpaper.presets.findIndex(p => p.id === preset.id);
      if (index !== -1) {
        wallpaper.presets[index] = preset;
        updateConfig({ wallpaper });
      }
      return wallpaper.presets;
    }
  );

  ipcMain.handle('wallpaper:deletePreset', (_event, id: string) => {
    const wallpaper = currentConfig.wallpaper;
    wallpaper.presets = wallpaper.presets.filter(p => p.id !== id);
    if (wallpaper.defaultPresetId === id) {
      wallpaper.defaultPresetId = null;
    }
    updateConfig({ wallpaper });
    return wallpaper.presets;
  });

  ipcMain.handle('wallpaper:setDefaultPreset', (_event, id: string | null) => {
    const wallpaper = currentConfig.wallpaper;
    wallpaper.defaultPresetId =
      id && wallpaper.presets.some(p => p.id === id) ? id : null;
    updateConfig({ wallpaper });
    return wallpaper.defaultPresetId;
  });

  ipcMain.handle('wallpaper:selectImage', async () => {
    try {
      const result = await dialog.showOpenDialog({
        properties: ['openFile'],
        filters: [
          {
            name: 'Images',
            extensions: ['png', 'jpg', 'jpeg', 'svg', 'webp'],
          },
        ],
      });

      if (result.canceled || result.filePaths.length === 0) {
        return null;
      }

      const filePath = result.filePaths[0];
      const imageBuffer = fs.readFileSync(filePath);
      const ext = path.extname(filePath).toLowerCase();

      const mimeType =
        ext === '.png'
          ? 'image/png'
          : ext === '.jpg' || ext === '.jpeg'
            ? 'image/jpeg'
            : ext === '.svg'
              ? 'image/svg+xml'
              : ext === '.webp'
                ? 'image/webp'
                : 'image/png';

      const base64 = imageBuffer.toString('base64');
      return `data:${mimeType};base64,${base64}`;
    } catch (error) {
      console.error('Failed to select image:', error);
      return null;
    }
  });

  ipcMain.handle(
    'storage:selectPath',
    async (_event, type: 'screenshots' | 'recordings') => {
      const defaultPath =
        type === 'screenshots'
          ? app.getPath('pictures')
          : app.getPath('videos');

      const result = await dialog.showOpenDialog({
        properties: ['openDirectory', 'createDirectory'],
        defaultPath,
        title: `Select ${type === 'screenshots' ? 'Screenshots' : 'Recordings'} Folder`,
      });

      if (result.canceled || result.filePaths.length === 0) {
        return null;
      }

      const selectedPath = result.filePaths[0];
      const validation = validateStoragePath(selectedPath);

      if (!validation.valid) {
        return { error: validation.error };
      }

      return { path: selectedPath };
    }
  );

  ipcMain.handle('storage:validatePattern', (_event, pattern: string) => {
    return validateNamingPattern(pattern);
  });

  ipcMain.handle(
    'storage:previewFilename',
    (_event, pattern: string, type: CaptureType) => {
      const extension = type === 'Screenshot' ? 'png' : 'mov';
      return generateFilename({ pattern, type, extension });
    }
  );

  ipcMain.handle('storage:getTokens', () => {
    return getAvailableTokens();
  });

  ipcMain.handle('storage:getDefaultPaths', () => {
    const storage = getConfig().storage;
    return {
      screenshots:
        storage.screenshotsPath ||
        path.join(app.getPath('pictures'), 'Poratake'),
      recordings:
        storage.recordingsPath || path.join(app.getPath('videos'), 'Poratake'),
    };
  });

  ipcMain.handle('cursor:selectImage', async () => {
    try {
      const result = await dialog.showOpenDialog({
        properties: ['openFile'],
        filters: [
          {
            name: 'Images',
            extensions: ['png', 'jpg', 'jpeg', 'svg', 'webp', 'gif'],
          },
        ],
      });

      if (result.canceled || result.filePaths.length === 0) {
        return null;
      }

      const filePath = result.filePaths[0];
      const imageBuffer = fs.readFileSync(filePath);
      const ext = path.extname(filePath).toLowerCase();

      const mimeType =
        ext === '.png'
          ? 'image/png'
          : ext === '.jpg' || ext === '.jpeg'
            ? 'image/jpeg'
            : ext === '.svg'
              ? 'image/svg+xml'
              : ext === '.webp'
                ? 'image/webp'
                : ext === '.gif'
                  ? 'image/gif'
                  : 'image/png';

      const base64 = imageBuffer.toString('base64');
      return `data:${mimeType};base64,${base64}`;
    } catch (error) {
      console.error('Failed to select cursor image:', error);
      return null;
    }
  });

  ipcMain.handle('wallpaper:getDesktopWallpaper', async () => {
    try {
      const result = await daemon.call<{ type: string; value: string }>(
        'desktop-wallpaper',
        'get'
      );

      if (!result) {
        return null;
      }

      if (result.type === 'data') {
        return result.value;
      }

      if (result.type === 'path') {
        const filePath = result.value;
        if (!fs.existsSync(filePath)) {
          console.error('Desktop wallpaper file not found:', filePath);
          return null;
        }

        return pathToFileURL(filePath).href;
      }

      return null;
    } catch (error) {
      console.error('Failed to get desktop wallpaper:', error);
      return null;
    }
  });
}

function validateStoragePath(dirPath: string): {
  valid: boolean;
  error?: string;
} {
  try {
    if (!fs.existsSync(dirPath)) {
      fs.mkdirSync(dirPath, { recursive: true });
    }

    const stats = fs.statSync(dirPath);
    if (!stats.isDirectory()) {
      return { valid: false, error: 'Selected path is not a directory' };
    }

    const testFile = path.join(dirPath, `.poratake-test-${Date.now()}`);
    fs.writeFileSync(testFile, '');
    fs.unlinkSync(testFile);

    return { valid: true };
  } catch (error) {
    return {
      valid: false,
      error:
        error instanceof Error ? error.message : 'Unable to access directory',
    };
  }
}
