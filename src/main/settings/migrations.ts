import path from 'path';
import type {
  AllInOneConfig,
  AppearanceConfig,
  CloudConfig,
  CustomBackground,
  CustomGradient,
  RecordingSettings,
  SettingsConfig,
  SettingsUiConfig,
  WallpaperPreset,
} from '@/types/settings.ts';
import { APP_THEME_PRESETS } from '@/types/theme';
import {
  DEFAULT_ALL_IN_ONE_CONFIG,
  DEFAULT_CLOUD_CONFIG,
  DEFAULT_PREVIEW_CONFIG,
  DEFAULT_REST_PROVIDER_CONFIG,
  DEFAULT_S3_PROVIDER_CONFIG,
  DEFAULT_SAVE_LOCATIONS_CONFIG,
  DEFAULT_SETTINGS,
  DEFAULT_STORAGE_CONFIG,
} from '@/types/settings.ts';

const APPEARANCE_MODES = new Set(['system', 'light', 'dark']);
const APP_THEME_IDS = new Set(APP_THEME_PRESETS.map(theme => theme.id));

type SavedAllInOneConfig = Partial<
  Omit<AllInOneConfig, 'lastTargets' | 'recording'>
> & {
  lastTargets?: Partial<AllInOneConfig['lastTargets']>;
  recording?: Partial<AllInOneConfig['recording']>;
};

interface SettingsMigrationOptions {
  cloud: CloudConfig;
  getPicturesPath: () => string;
  getVideosPath: () => string;
  wallpaper: SettingsConfig['wallpaper'];
}

export function createDefaultSettings(): SettingsConfig {
  return structuredClone(DEFAULT_SETTINGS);
}

export function migrateAppearanceConfig(
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

export function migrateAllInOneConfig(
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

export function getSettingsUiConfig(
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

export function migrateStorageConfig(
  savedStorage: Partial<SettingsConfig['storage']> | undefined,
  getPicturesPath: () => string,
  getVideosPath: () => string
): SettingsConfig['storage'] {
  const storage = { ...DEFAULT_STORAGE_CONFIG, ...savedStorage };

  return {
    ...storage,
    screenshotsPath:
      storage.screenshotsPath || path.join(getPicturesPath(), 'Poratake'),
    recordingsPath:
      storage.recordingsPath || path.join(getVideosPath(), 'Poratake'),
  };
}

export function migrateWallpaperConfig(
  savedWallpaper?: SettingsConfig['wallpaper']
): SettingsConfig['wallpaper'] {
  const base = {
    ...DEFAULT_SETTINGS.wallpaper,
    ...savedWallpaper,
  };

  const presets = Array.isArray(base.presets)
    ? base.presets.filter(
        (preset): preset is WallpaperPreset =>
          !!preset && typeof preset === 'object'
      )
    : [];
  const customBackgrounds = Array.isArray(base.customBackgrounds)
    ? base.customBackgrounds.filter(
        (background): background is CustomBackground =>
          !!background && typeof background === 'object'
      )
    : [];
  const defaultPresetId = presets.some(p => p.id === base.defaultPresetId)
    ? base.defaultPresetId
    : null;

  if (customBackgrounds.length > 0) {
    return {
      customBackgrounds,
      presets,
      defaultPresetId,
    };
  }

  const savedGradients = (
    savedWallpaper as { customGradients?: CustomGradient[] }
  )?.customGradients;
  const legacyGradients = Array.isArray(savedGradients) ? savedGradients : [];
  const migratedBackgrounds: CustomBackground[] = legacyGradients
    .filter(
      gradient =>
        !!gradient &&
        Array.isArray(gradient.colors) &&
        gradient.angle !== undefined
    )
    .map(gradient => ({
      id: gradient.id,
      type: 'gradient' as const,
      data: {
        colors: gradient.colors!,
        angle: gradient.angle!,
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
    return structuredClone(DEFAULT_CLOUD_CONFIG);
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
  const restHeaders = Array.isArray(restSource.headers)
    ? restSource.headers.filter(
        (header): header is { key: string; value: string } =>
          !!header &&
          typeof header === 'object' &&
          typeof (header as Record<string, unknown>).key === 'string' &&
          typeof (header as Record<string, unknown>).value === 'string'
      )
    : DEFAULT_REST_PROVIDER_CONFIG.headers;
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
    typeof raw.enabled === 'boolean'
      ? raw.enabled
      : hasLegacyFlatS3 && hasConfiguredS3
        ? true
        : DEFAULT_CLOUD_CONFIG.enabled;

  return {
    enabled,
    activeProvider,
    s3: {
      ...DEFAULT_S3_PROVIDER_CONFIG,
      ...s3Source,
      secretAccessKey:
        typeof s3Source.secretAccessKey === 'string'
          ? s3Source.secretAccessKey
          : DEFAULT_S3_PROVIDER_CONFIG.secretAccessKey,
    },
    rest: {
      ...DEFAULT_REST_PROVIDER_CONFIG,
      ...restSource,
      headers: restHeaders,
    },
  };
}

export function mergeCloudConfig(
  cloud: SettingsConfig['cloud'],
  updates?: SettingsConfig['cloud']
): SettingsConfig['cloud'] {
  return {
    ...cloud,
    ...updates,
    s3: { ...cloud.s3, ...updates?.s3 },
    rest: {
      ...cloud.rest,
      ...updates?.rest,
      headers: updates?.rest?.headers ?? cloud.rest.headers,
    },
  };
}

export function mergeShortcutsConfig(
  base: SettingsConfig['shortcuts'],
  patch?: Partial<SettingsConfig['shortcuts']>
): SettingsConfig['shortcuts'] {
  return {
    screenshot: { ...base.screenshot, ...patch?.screenshot },
    captureText: patch?.captureText ?? base.captureText,
    scanQRCode: patch?.scanQRCode ?? base.scanQRCode,
    timerCapture: patch?.timerCapture ?? base.timerCapture,
    scrollCapture: patch?.scrollCapture ?? base.scrollCapture,
    recording: { ...base.recording, ...patch?.recording },
    history: patch?.history ?? base.history,
    allInOne: patch?.allInOne ?? base.allInOne,
    openInEditor: patch?.openInEditor ?? base.openInEditor,
    clipboardInEditor: patch?.clipboardInEditor ?? base.clipboardInEditor,
    editor: { ...base.editor, ...patch?.editor },
    editorActions: { ...base.editorActions, ...patch?.editorActions },
    videoEditorSidebar: {
      ...base.videoEditorSidebar,
      ...patch?.videoEditorSidebar,
    },
  };
}

export function migrateSettingsConfig(
  savedConfig: Partial<SettingsConfig>,
  options: SettingsMigrationOptions
): SettingsConfig {
  const defaults = createDefaultSettings();

  return {
    appearance: migrateAppearanceConfig(savedConfig.appearance),
    general: { ...defaults.general, ...savedConfig.general },
    screenshot: { ...defaults.screenshot, ...savedConfig.screenshot },
    shortcuts: mergeShortcutsConfig(defaults.shortcuts, savedConfig.shortcuts),
    editor: { ...defaults.editor, ...savedConfig.editor },
    wallpaper: options.wallpaper,
    history: { ...defaults.history, ...savedConfig.history },
    onboarding: { ...defaults.onboarding, ...savedConfig.onboarding },
    cloud: options.cloud,
    recording: {
      ...defaults.recording,
      ...savedConfig.recording,
      camera: {
        ...defaults.recording.camera,
        ...savedConfig.recording?.camera,
      },
    },
    storage: migrateStorageConfig(
      savedConfig.storage,
      options.getPicturesPath,
      options.getVideosPath
    ),
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
      ...defaults.scrollCapture,
      ...savedConfig.scrollCapture,
    },
  };
}
