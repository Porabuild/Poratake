import { describe, it, expect, vi } from 'vitest';

const mockSafeStorage = vi.hoisted(() => ({
  isEncryptionAvailable: vi.fn(() => true),
  encryptString: vi.fn((value: string) => Buffer.from(`encrypted:${value}`)),
  decryptString: vi.fn((value: Buffer) =>
    value.toString().replace(/^encrypted:/, '')
  ),
}));

vi.mock('electron', () => ({
  app: {
    setLoginItemSettings: vi.fn(),
    getPath: () => '/tmp',
  },
  BrowserWindow: { getAllWindows: () => [] },
  dialog: {},
  ipcMain: { handle: vi.fn() },
  safeStorage: mockSafeStorage,
}));

vi.mock('@/main/daemon', () => ({ daemon: { call: vi.fn() } }));

vi.mock('@/main/utils/paths.ts', () => ({
  getConfigDir: () => '/tmp/poratake-test',
  getConfigFilePath: () => '/tmp/poratake-test/config.json',
}));

vi.mock('@/main/utils/env.ts', () => ({ getAppVersion: () => '0.0.0' }));

vi.mock('@/main/utils/filename-generator', () => ({
  generateFilename: vi.fn(),
  validateNamingPattern: vi.fn(),
  getAvailableTokens: vi.fn(),
}));

import { migrateCloudConfig } from '@/main/settings';
import {
  hasUnprotectedCloudSecrets,
  protectCloudSecrets,
  revealCloudSecrets,
} from '@/main/settings/cloud-secrets';

describe('migrateCloudConfig', () => {
  it('returns disabled S3 defaults when input is undefined', () => {
    const result = migrateCloudConfig(undefined);
    expect(result.activeProvider).toBe('s3');
    expect(result.enabled).toBe(false);
    expect(result.s3.endpoint).toBe('');
    expect(result.rest.url).toBe('');
    expect(result.rest.fileFieldName).toBe('file');
  });

  it('keeps fresh empty configs on the S3 defaults', () => {
    const result = migrateCloudConfig({});
    expect(result.activeProvider).toBe('s3');
    expect(result.enabled).toBe(false);
  });

  it('moves unconfigured legacy REST defaults to S3 defaults', () => {
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
    expect(result.activeProvider).toBe('s3');
    expect(result.enabled).toBe(false);
  });

  it('moves unconfigured legacy flat S3 defaults to S3 defaults', () => {
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
    expect(result.activeProvider).toBe('s3');
    expect(result.enabled).toBe(false);
  });

  it('keeps a configured legacy flat S3 config enabled when enabled was unset', () => {
    const result = migrateCloudConfig({
      endpoint: 'https://s3.amazonaws.com',
      region: 'us-east-1',
      bucket: 'b',
      accessKeyId: 'AK',
      secretAccessKey: 'SK',
      pathPrefix: '',
      customDomain: '',
    });
    expect(result.activeProvider).toBe('s3');
    expect(result.enabled).toBe(true);
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

  it('coerces unknown activeProvider back to S3 defaults when no provider is configured', () => {
    const result = migrateCloudConfig({ activeProvider: 'ftp' });
    expect(result.activeProvider).toBe('s3');
    expect(result.enabled).toBe(false);
  });

  it('coerces unknown activeProvider to s3 when legacy flat fields are present', () => {
    const result = migrateCloudConfig({
      activeProvider: 'ftp',
      endpoint: 'https://s3.amazonaws.com',
    });
    expect(result.activeProvider).toBe('s3');
  });

  it('protects and reveals persisted credential values', () => {
    const cloud = migrateCloudConfig({
      enabled: true,
      activeProvider: 'rest',
      s3: { secretAccessKey: 's3-secret' },
      rest: {
        url: 'https://example.com/upload',
        headers: [{ key: 'Authorization', value: 'Bearer secret' }],
      },
    });

    expect(hasUnprotectedCloudSecrets(cloud)).toBe(true);
    const protectedCloud = protectCloudSecrets(cloud, {
      restHeaders: [],
    }).cloud;
    expect(protectedCloud.s3.secretAccessKey).toMatch(
      /^poratake-safe-storage:v1:/
    );
    expect(protectedCloud.rest.headers[0].value).toMatch(
      /^poratake-safe-storage:v1:/
    );
    expect(hasUnprotectedCloudSecrets(protectedCloud)).toBe(false);
    expect(revealCloudSecrets(protectedCloud).cloud).toEqual(cloud);
  });

  it('protects a plaintext credential that starts with the storage marker', () => {
    const cloud = migrateCloudConfig({
      s3: { secretAccessKey: 'poratake-safe-storage:v1:literal-secret' },
    });

    const protectedCloud = protectCloudSecrets(cloud, {
      restHeaders: [],
    }).cloud;

    expect(protectedCloud.s3.secretAccessKey).not.toBe(
      cloud.s3.secretAccessKey
    );
    expect(revealCloudSecrets(protectedCloud).cloud).toEqual(cloud);
  });

  it('sanitizes malformed REST headers', () => {
    const result = migrateCloudConfig({
      rest: {
        headers: { Authorization: 'secret' },
      },
    });

    expect(result.rest.headers).toEqual([]);
  });

  it('sanitizes a malformed S3 secret', () => {
    const result = migrateCloudConfig({
      s3: { secretAccessKey: 42 },
    });

    expect(result.s3.secretAccessKey).toBe('');
  });

  it('retains encrypted values when OS encryption is unavailable', () => {
    const cloud = migrateCloudConfig({
      s3: { secretAccessKey: 's3-secret' },
      rest: {
        headers: [
          { key: 'Authorization', value: 'Bearer secret' },
          { key: 'X-Api-Key', value: 'api-secret' },
        ],
      },
    });
    const protectedResult = protectCloudSecrets(cloud, { restHeaders: [] });
    mockSafeStorage.isEncryptionAvailable.mockReturnValue(false);
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const revealedResult = revealCloudSecrets(protectedResult.cloud);
    expect(revealedResult.cloud.s3.secretAccessKey).toBe('');
    expect(revealedResult.cloud.rest.headers[0].value).toBe('');

    const persistedResult = protectCloudSecrets(
      revealedResult.cloud,
      revealedResult.retained
    );
    expect(persistedResult.cloud.s3.secretAccessKey).toBe(
      protectedResult.cloud.s3.secretAccessKey
    );
    expect(persistedResult.cloud.rest.headers[0].value).toBe(
      protectedResult.cloud.rest.headers[0].value
    );

    const shiftedCloud = {
      ...revealedResult.cloud,
      rest: {
        ...revealedResult.cloud.rest,
        headers: [revealedResult.cloud.rest.headers[1]],
      },
    };
    const shiftedResult = protectCloudSecrets(
      shiftedCloud,
      revealedResult.retained
    );
    expect(shiftedResult.cloud.rest.headers[0].value).toBe(
      protectedResult.cloud.rest.headers[1].value
    );

    const renamedCloud = {
      ...revealedResult.cloud,
      rest: {
        ...revealedResult.cloud.rest,
        headers: [
          {
            ...revealedResult.cloud.rest.headers[0],
            key: 'X-Authorization',
          },
          revealedResult.cloud.rest.headers[1],
        ],
      },
    };
    const renamedResult = protectCloudSecrets(
      renamedCloud,
      revealedResult.retained
    );
    expect(renamedResult.cloud.rest.headers[0].value).toBe(
      protectedResult.cloud.rest.headers[0].value
    );

    errorSpy.mockRestore();
    mockSafeStorage.isEncryptionAvailable.mockReturnValue(true);
  });

  it('reports when a new secret cannot be protected', () => {
    mockSafeStorage.isEncryptionAvailable.mockReturnValue(false);
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const cloud = migrateCloudConfig({
      s3: { secretAccessKey: 'new-secret' },
    });

    const result = protectCloudSecrets(cloud, { restHeaders: [] });

    expect(result.failedToProtect).toBe(true);
    expect(result.cloud.s3.secretAccessKey).toBe('');
    errorSpy.mockRestore();
    mockSafeStorage.isEncryptionAvailable.mockReturnValue(true);
  });
});
