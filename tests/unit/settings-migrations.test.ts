import path from 'path';
import { describe, expect, it, vi } from 'vitest';
import {
  createDefaultSettings,
  getSettingsUiConfig,
  migrateAllInOneConfig,
  migrateAppearanceConfig,
  migrateCloudConfig,
  migrateSettingsConfig,
  migrateStorageConfig,
  migrateWallpaperConfig,
} from '@/main/settings/migrations';
import { DEFAULT_SETTINGS } from '@/types/settings';

describe('settings migrations', () => {
  it('normalizes appearance values at the migration boundary', () => {
    expect(
      migrateAppearanceConfig({ mode: 'invalid', theme: 'missing' })
    ).toEqual(DEFAULT_SETTINGS.appearance);
    expect(
      migrateAppearanceConfig({ mode: 'light', theme: 'catppuccin' })
    ).toEqual({ mode: 'light', theme: 'catppuccin' });
  });

  it('preserves explicit false recording choices in all-in-one settings', () => {
    const migrated = migrateAllInOneConfig(
      {
        recording: {
          systemAudio: false,
          micEnabled: false,
          cameraEnabled: false,
        },
      },
      {
        ...DEFAULT_SETTINGS.recording,
        systemAudio: true,
        micEnabled: true,
        camera: { ...DEFAULT_SETTINGS.recording.camera, enabled: true },
      }
    );

    expect(migrated.recording).toEqual({
      systemAudio: false,
      micEnabled: false,
      cameraEnabled: false,
    });
  });

  it('falls back to recording choices when all-in-one choices are absent', () => {
    const migrated = migrateAllInOneConfig(undefined, {
      ...DEFAULT_SETTINGS.recording,
      systemAudio: false,
      micEnabled: true,
      camera: { ...DEFAULT_SETTINGS.recording.camera, enabled: true },
    });

    expect(migrated.recording).toEqual({
      systemAudio: false,
      micEnabled: true,
      cameraEnabled: true,
    });
  });

  it('converts legacy gradients and drops a missing default preset', () => {
    const migrated = migrateWallpaperConfig({
      customBackgrounds: [],
      presets: [],
      defaultPresetId: 'missing',
      customGradients: [
        {
          id: 'legacy',
          name: 'Legacy',
          colors: ['#111111', '#eeeeee'],
          angle: 45,
        },
      ],
    } as never);

    expect(migrated).toEqual({
      customBackgrounds: [
        {
          id: 'legacy',
          type: 'gradient',
          data: { colors: ['#111111', '#eeeeee'], angle: 45 },
        },
      ],
      presets: [],
      defaultPresetId: null,
    });
  });

  it('repairs empty storage paths without replacing configured paths', () => {
    const getPicturesPath = vi.fn(() => 'C:\\Pictures');
    const getVideosPath = vi.fn(() => 'C:\\Videos');

    expect(
      migrateStorageConfig(
        { screenshotsPath: '', recordingsPath: 'D:\\Recordings' },
        getPicturesPath,
        getVideosPath
      )
    ).toMatchObject({
      screenshotsPath: path.join('C:\\Pictures', 'Poratake'),
      recordingsPath: 'D:\\Recordings',
    });
    expect(getPicturesPath).toHaveBeenCalledOnce();
    expect(getVideosPath).not.toHaveBeenCalled();
  });

  it('deeply merges nested defaults while preserving saved false values', () => {
    const wallpaper = migrateWallpaperConfig();
    const cloud = migrateCloudConfig(undefined);
    const migrated = migrateSettingsConfig(
      {
        screenshot: { freezeScreen: false },
        shortcuts: {
          screenshot: { area: 'Custom+Area' },
          recording: { area: 'Custom+Record' },
        },
        recording: {
          systemAudio: false,
          camera: { enabled: true },
        },
      } as never,
      {
        cloud,
        wallpaper,
        getPicturesPath: () => 'C:\\Pictures',
        getVideosPath: () => 'C:\\Videos',
      }
    );

    expect(migrated.screenshot.freezeScreen).toBe(false);
    expect(migrated.shortcuts.screenshot.area).toBe('Custom+Area');
    expect(migrated.shortcuts.screenshot.screen).toBe(
      DEFAULT_SETTINGS.shortcuts.screenshot.screen
    );
    expect(migrated.shortcuts.screenshot.window).toBe(
      DEFAULT_SETTINGS.shortcuts.screenshot.window
    );
    expect(migrated.shortcuts.recording.area).toBe('Custom+Record');
    expect(migrated.shortcuts.recording.screen).toBe(
      DEFAULT_SETTINGS.shortcuts.recording.screen
    );
    expect(migrated.shortcuts.recording.window).toBe(
      DEFAULT_SETTINGS.shortcuts.recording.window
    );
    expect(migrated.recording.systemAudio).toBe(false);
    expect(migrated.recording.camera.enabled).toBe(true);
    expect(migrated.recording.camera.shape).toBe(
      DEFAULT_SETTINGS.recording.camera.shape
    );
  });

  it('redacts renderer credentials and wallpaper without mutating config', () => {
    const config = createDefaultSettings();
    config.cloud.s3.accessKeyId = 'access';
    config.cloud.s3.secretAccessKey = 'secret';
    config.cloud.rest.headers = [
      { key: 'Authorization', value: 'Bearer token' },
    ];
    config.wallpaper.customBackgrounds = [
      { id: 'private', type: 'image', data: { imageUrl: 'file:///private' } },
    ];

    const rendererConfig = getSettingsUiConfig(config, false);

    expect(rendererConfig).not.toHaveProperty('wallpaper');
    expect(rendererConfig.cloud.s3).toMatchObject({
      accessKeyId: '',
      secretAccessKey: '',
    });
    expect(rendererConfig.cloud.rest.headers[0].value).toBe('');
    expect(config.cloud.s3.secretAccessKey).toBe('secret');
    expect(config.cloud.rest.headers[0].value).toBe('Bearer token');
    expect(config.wallpaper.customBackgrounds).toHaveLength(1);
  });
});
