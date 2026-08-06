import { describe, it, expect, vi } from 'vitest';

vi.mock('electron', () => ({
  app: {
    setLoginItemSettings: vi.fn(),
    getPath: () => '/tmp',
  },
  BrowserWindow: { getAllWindows: () => [] },
  dialog: {},
  ipcMain: { handle: vi.fn() },
}));

vi.mock('@/main/daemon', () => ({ daemon: { call: vi.fn() } }));

vi.mock('@/main/utils/paths.ts', () => ({
  getConfigDir: () => '/tmp/capty-test',
  getConfigFilePath: () => '/tmp/capty-test/config.json',
}));

vi.mock('@/main/utils/env.ts', () => ({ getAppVersion: () => '0.0.0' }));

vi.mock('@/main/utils/filename-generator', () => ({
  generateFilename: vi.fn(),
  validateNamingPattern: vi.fn(),
  getAvailableTokens: vi.fn(),
}));

import { migrateCloudConfig } from '@/main/settings';

describe('migrateCloudConfig', () => {
  it('returns enabled Capty Cloud defaults when input is undefined', () => {
    const result = migrateCloudConfig(undefined);
    expect(result.activeProvider).toBe('capty');
    expect(result.enabled).toBe(true);
    expect(result.s3.endpoint).toBe('');
    expect(result.rest.url).toBe('');
    expect(result.rest.fileFieldName).toBe('file');
  });

  it('moves fresh empty configs to Capty Cloud', () => {
    const result = migrateCloudConfig({});
    expect(result.activeProvider).toBe('capty');
    expect(result.enabled).toBe(true);
  });

  it('moves unconfigured legacy REST defaults to Capty Cloud', () => {
    const result = migrateCloudConfig({
      enabled: false,
      activeProvider: 'rest',
      rest: {
        url: '',
        headers: [],
        fileFieldName: 'file',
        responseIsPlainText: false,
        responseUrlPath: '',
      },
    });
    expect(result.activeProvider).toBe('capty');
    expect(result.enabled).toBe(true);
  });

  it('moves unconfigured legacy flat S3 defaults to Capty Cloud', () => {
    const result = migrateCloudConfig({
      enabled: false,
      endpoint: '',
      region: '',
      bucket: '',
      accessKeyId: '',
      secretAccessKey: '',
      pathPrefix: '',
      customDomain: '',
    });
    expect(result.activeProvider).toBe('capty');
    expect(result.enabled).toBe(true);
  });

  it('preserves an explicitly disabled Capty Cloud config', () => {
    const result = migrateCloudConfig({
      enabled: false,
      activeProvider: 'capty',
    });
    expect(result.activeProvider).toBe('capty');
    expect(result.enabled).toBe(false);
  });

  it('migrates legacy flat S3 fields into cloud.s3', () => {
    const legacy = {
      enabled: true,
      endpoint: 'https://s3.amazonaws.com',
      region: 'us-east-1',
      bucket: 'b',
      accessKeyId: 'AK',
      secretAccessKey: 'SK',
      pathPrefix: 'p/',
      customDomain: 'https://cdn.example.com',
    };
    const result = migrateCloudConfig(legacy);
    expect(result.enabled).toBe(true);
    expect(result.activeProvider).toBe('s3');
    expect(result.s3).toMatchObject({
      endpoint: 'https://s3.amazonaws.com',
      region: 'us-east-1',
      bucket: 'b',
      accessKeyId: 'AK',
      secretAccessKey: 'SK',
      pathPrefix: 'p/',
      customDomain: 'https://cdn.example.com',
    });
  });

  it('keeps nested s3 config when already in new shape', () => {
    const next = {
      enabled: true,
      activeProvider: 'rest',
      s3: {
        endpoint: 'https://s3.amazonaws.com',
        region: 'us-east-1',
        bucket: 'b',
        accessKeyId: 'AK',
        secretAccessKey: 'SK',
        pathPrefix: '',
        customDomain: '',
      },
      rest: {
        url: 'https://api.example.com/upload',
        headers: [{ key: 'A', value: '1' }],
        fileFieldName: 'image',
        responseIsPlainText: true,
        responseUrlPath: '',
      },
    };
    const result = migrateCloudConfig(next);
    expect(result.activeProvider).toBe('rest');
    expect(result.s3.bucket).toBe('b');
    expect(result.rest.url).toBe('https://api.example.com/upload');
    expect(result.rest.headers).toEqual([{ key: 'A', value: '1' }]);
    expect(result.rest.fileFieldName).toBe('image');
    expect(result.rest.responseIsPlainText).toBe(true);
  });

  it('coerces unknown activeProvider back to Capty Cloud when no provider is configured', () => {
    const result = migrateCloudConfig({ activeProvider: 'ftp' });
    expect(result.activeProvider).toBe('capty');
  });

  it('coerces unknown activeProvider to s3 when legacy flat fields are present', () => {
    const result = migrateCloudConfig({
      activeProvider: 'ftp',
      endpoint: 'https://s3.amazonaws.com',
    });
    expect(result.activeProvider).toBe('s3');
  });
});
