import { app } from 'electron';
import fs from 'fs';
import path from 'path';
import type { SettingsConfig } from '@/types/settings.ts';
import { getConfigDir, getConfigFilePath } from '@/main/utils/paths.ts';
import {
  hasUnprotectedCloudSecrets,
  protectCloudSecrets,
  revealCloudSecrets,
  type RetainedCloudSecrets,
} from './cloud-secrets.ts';
import {
  migrateWallpaperAssets,
  pruneWallpaperAssets,
} from './wallpaper-assets.ts';
import {
  createDefaultSettings,
  mergeCloudConfig,
  migrateAppearanceConfig,
  migrateCloudConfig,
  migrateSettingsConfig,
  migrateWallpaperConfig,
  mergeShortcutsConfig,
} from './migrations.ts';

const CONFIG_DIR = getConfigDir();
const CONFIG_FILE = getConfigFilePath();
const CONFIG_WRITE_DEBOUNCE_MS = 150;

let currentConfig: SettingsConfig = createDefaultSettings();
let configLoaded = false;
let configWriteTimer: NodeJS.Timeout | null = null;
let configWritePending = false;
let configWriteInFlight = false;
let configWriteGeneration = 0;
let configWriteSequence = 0;
let configWriteQueue: Promise<void> = Promise.resolve();
let retainedCloudSecrets: RetainedCloudSecrets = { restHeaders: [] };
const configUpdateListeners = new Set<
  (updates: Partial<SettingsConfig>) => void
>();

export function onConfigUpdated(
  listener: (updates: Partial<SettingsConfig>) => void
): () => void {
  configUpdateListeners.add(listener);
  return () => configUpdateListeners.delete(listener);
}

function ensureConfigDir(): void {
  if (!fs.existsSync(CONFIG_DIR)) {
    fs.mkdirSync(CONFIG_DIR, { recursive: true });
  }
}

function preserveCorruptConfig(fileContent: string): void {
  const parsedPath = path.parse(CONFIG_FILE);
  const recoveryPath = path.join(
    parsedPath.dir,
    `${parsedPath.name}.corrupt-${Date.now()}${parsedPath.ext}`
  );

  try {
    try {
      const parsed = JSON.parse(fileContent) as unknown;
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        const sanitized = { ...(parsed as Record<string, unknown>) };
        Reflect.deleteProperty(sanitized, 'cloud');
        fs.writeFileSync(recoveryPath, JSON.stringify(sanitized, null, 2));
        fs.unlinkSync(CONFIG_FILE);
        return;
      }
    } catch {}
    fs.renameSync(CONFIG_FILE, recoveryPath);
  } catch (error) {
    console.error('Failed to preserve corrupt config:', error);
  }
}

export function loadConfig(): SettingsConfig {
  if (configLoaded) {
    return currentConfig;
  }

  let fileContent: string | null = null;
  try {
    ensureConfigDir();
    if (fs.existsSync(CONFIG_FILE)) {
      fileContent = fs.readFileSync(CONFIG_FILE, 'utf-8');
      const savedConfig = JSON.parse(fileContent) as Partial<SettingsConfig>;
      const migratedCloud = migrateCloudConfig(savedConfig.cloud);
      const shouldProtectCloudSecrets =
        hasUnprotectedCloudSecrets(migratedCloud);
      const revealedCloud = revealCloudSecrets(migratedCloud);
      retainedCloudSecrets = revealedCloud.retained;
      const wallpaperMigration = migrateWallpaperAssets(
        migrateWallpaperConfig(savedConfig.wallpaper)
      );
      currentConfig = migrateSettingsConfig(savedConfig, {
        cloud: revealedCloud.cloud,
        getPicturesPath: () => app.getPath('pictures'),
        getVideosPath: () => app.getPath('videos'),
        wallpaper: wallpaperMigration.wallpaper,
      });
      if (shouldProtectCloudSecrets || wallpaperMigration.migrated) {
        saveConfig(currentConfig);
      }
    } else {
      currentConfig = createDefaultSettings();
      retainedCloudSecrets = { restHeaders: [] };
      saveConfig(currentConfig);
    }
    pruneWallpaperAssets(currentConfig.wallpaper);
    configLoaded = true;
  } catch (error) {
    console.error('Failed to load config:', error);
    if (fileContent !== null && fs.existsSync(CONFIG_FILE)) {
      preserveCorruptConfig(fileContent);
    }
    currentConfig = createDefaultSettings();
    retainedCloudSecrets = { restHeaders: [] };
    configLoaded = true;
  }
  return currentConfig;
}

function writeConfigFileSync(): void {
  configWriteGeneration += 1;
  const tempFile = `${CONFIG_FILE}.flush.tmp`;
  fs.writeFileSync(tempFile, serializeConfig(), 'utf-8');
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
    await fs.promises.writeFile(tempFile, serializeConfig(), 'utf-8');
    if (generation !== configWriteGeneration) {
      await fs.promises.unlink(tempFile).catch(() => {});
      return;
    }
    await fs.promises.rename(tempFile, CONFIG_FILE);
  } finally {
    configWriteInFlight = false;
  }
}

function serializeConfig(): string {
  const protectedCloud = protectCloudSecrets(
    currentConfig.cloud,
    retainedCloudSecrets
  );
  retainedCloudSecrets = protectedCloud.retained;
  return JSON.stringify(
    {
      ...currentConfig,
      cloud: protectedCloud.cloud,
    },
    null,
    2
  );
}

export function flushConfigFile(): void {
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
    shortcuts: mergeShortcutsConfig(currentConfig.shortcuts, updates.shortcuts),
    editor: { ...currentConfig.editor, ...updates.editor },
    wallpaper: { ...currentConfig.wallpaper, ...updates.wallpaper },
    history: { ...currentConfig.history, ...updates.history },
    onboarding: { ...currentConfig.onboarding, ...updates.onboarding },
    cloud: mergeCloudConfig(currentConfig.cloud, updates.cloud),
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

export function resetConfig(): SettingsConfig {
  currentConfig = createDefaultSettings();
  retainedCloudSecrets = { restHeaders: [] };
  saveConfig(currentConfig);
  configUpdateListeners.forEach(listener => listener(currentConfig));
  return currentConfig;
}

export function retainCloudUpdate(updates: SettingsConfig['cloud']): boolean {
  const protectedCloud = protectCloudSecrets(
    mergeCloudConfig(getConfig().cloud, updates),
    retainedCloudSecrets
  );
  if (protectedCloud.failedToProtect) {
    return false;
  }

  retainedCloudSecrets = protectedCloud.retained;
  return true;
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

export function applyLoginItemSetting(): void {
  const config = getConfig();
  try {
    app.setLoginItemSettings({
      openAtLogin: config.general.startOnLogin,
    });
  } catch (error) {
    console.warn('Failed to apply login item setting:', error);
  }
}
